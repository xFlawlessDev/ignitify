use std::{
    path::Path as FsPath,
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use ignitify_control_plane::AgeCipher;
use ignitify_db::{NewRemoteServer, RemoteServerRecord, RemoteServerUpdate};
use serde::{Deserialize, Serialize};
use tokio::{fs, process::Command, time::timeout};
use url::Url;
use uuid::Uuid;

use crate::{
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

const MAX_SSH_SECRET_BYTES: usize = 256 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoteServerRequest {
    name: String,
    host: String,
    port: u16,
    username: String,
    deploy_path: String,
    #[serde(default)]
    private_key: Option<String>,
    #[serde(default)]
    public_key: Option<String>,
    #[serde(default)]
    known_hosts: Option<String>,
    is_default: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct RemoteServerResponse {
    id: String,
    name: String,
    host: String,
    port: i64,
    username: String,
    deploy_path: String,
    private_key_configured: bool,
    public_key_configured: bool,
    known_hosts_configured: bool,
    is_default: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct RemoteServerCheckResponse {
    connected: bool,
    latency_ms: u64,
}

impl From<RemoteServerRecord> for RemoteServerResponse {
    fn from(value: RemoteServerRecord) -> Self {
        Self {
            id: value.id,
            name: value.name,
            host: value.host,
            port: value.port,
            username: value.username,
            deploy_path: value.deploy_path,
            private_key_configured: true,
            public_key_configured: value.public_key_configured,
            known_hosts_configured: true,
            is_default: value.is_default,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RemoteServerResponse>>, ApiError> {
    require_admin(&state, &headers).await?;
    let records = state
        .database
        .remote_servers()
        .list()
        .await?
        .into_iter()
        .map(RemoteServerResponse::from)
        .collect();
    Ok(Json(records))
}

pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RemoteServerRequest>,
) -> Result<(StatusCode, Json<RemoteServerResponse>), ApiError> {
    require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let record = state
        .database
        .remote_servers()
        .create(encrypt_create_request(&state, request)?)
        .await?;
    wake_worker(&state);
    Ok((StatusCode::CREATED, Json(record.into())))
}

pub(crate) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(server_id): Path<String>,
    Json(request): Json<RemoteServerRequest>,
) -> Result<Json<RemoteServerResponse>, ApiError> {
    require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let record = state
        .database
        .remote_servers()
        .update(&server_id, encrypt_update_request(&state, request)?)
        .await?
        .ok_or(ApiError::NotFound)?;
    wake_worker(&state);
    Ok(Json(record.into()))
}

pub(crate) async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(server_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    if !state.database.remote_servers().delete(&server_id).await? {
        return Err(ApiError::NotFound);
    }
    wake_worker(&state);
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn make_default(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(server_id): Path<String>,
) -> Result<Json<RemoteServerResponse>, ApiError> {
    require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let record = state
        .database
        .remote_servers()
        .set_default(&server_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    wake_worker(&state);
    Ok(Json(record.into()))
}

pub(crate) async fn check(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(server_id): Path<String>,
) -> Result<Json<RemoteServerCheckResponse>, ApiError> {
    require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let connection = state
        .database
        .remote_servers()
        .connection(&server_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let cipher = provider_cipher(&state)?;
    let private_key = cipher
        .decrypt(&connection.private_key_ciphertext)
        .map_err(|_| ApiError::RemoteServerCheckFailed)?;
    let known_hosts = cipher
        .decrypt(&connection.known_hosts_ciphertext)
        .map_err(|_| ApiError::RemoteServerCheckFailed)?;
    let directory = std::env::temp_dir().join(format!("ignitify-ssh-check-{}", Uuid::new_v4()));
    fs::create_dir(&directory)
        .await
        .map_err(|_| ApiError::RemoteServerCheckFailed)?;
    if let Err(error) = set_directory_permissions(&directory).await {
        let _ = fs::remove_dir_all(&directory).await;
        return Err(error);
    }
    let private_key_path = directory.join("id_key");
    let known_hosts_path = directory.join("known_hosts");
    let result = async {
        write_secret_file(&private_key_path, private_key.as_ref()).await?;
        write_secret_file(&known_hosts_path, known_hosts.as_ref()).await?;
        let started = Instant::now();
        let port = connection.port.to_string();
        let private_key_arg = private_key_path.to_string_lossy().into_owned();
        let known_hosts_option = format!("UserKnownHostsFile={}", known_hosts_path.display());
        let global_known_hosts_option = format!(
            "GlobalKnownHostsFile={}",
            if cfg!(windows) { "NUL" } else { "/dev/null" }
        );
        let target = format!("{}@{}", connection.username, connection.host);
        let status = timeout(
            Duration::from_secs(15),
            Command::new("ssh")
                .kill_on_drop(true)
                .args([
                    "-F",
                    "none",
                    "-i",
                    private_key_arg.as_str(),
                    "-o",
                    "IdentitiesOnly=yes",
                    "-o",
                    "PasswordAuthentication=no",
                    "-o",
                    "BatchMode=yes",
                    "-o",
                    "StrictHostKeyChecking=yes",
                    "-o",
                    known_hosts_option.as_str(),
                    "-o",
                    global_known_hosts_option.as_str(),
                    "-o",
                    "ConnectTimeout=10",
                    "-p",
                    port.as_str(),
                    target.as_str(),
                    "true",
                ])
                .env("LANG", "C")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status(),
        )
        .await
        .map_err(|_| ApiError::RemoteServerCheckFailed)?
        .map_err(|_| ApiError::RemoteServerCheckFailed)?;
        if !status.success() {
            return Err(ApiError::RemoteServerCheckFailed);
        }
        Ok(RemoteServerCheckResponse {
            connected: true,
            latency_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        })
    }
    .await;
    let _ = fs::remove_dir_all(&directory).await;
    result.map(Json)
}

async fn write_secret_file(path: &FsPath, contents: &[u8]) -> Result<(), ApiError> {
    fs::write(path, contents)
        .await
        .map_err(|_| ApiError::RemoteServerCheckFailed)?;
    set_file_permissions(path).await
}

async fn set_directory_permissions(path: &FsPath) -> Result<(), ApiError> {
    #[cfg(unix)]
    {
        fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .await
            .map_err(|_| ApiError::RemoteServerCheckFailed)?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

async fn set_file_permissions(path: &FsPath) -> Result<(), ApiError> {
    #[cfg(unix)]
    {
        fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .await
            .map_err(|_| ApiError::RemoteServerCheckFailed)?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn encrypt_create_request(
    state: &AppState,
    request: RemoteServerRequest,
) -> Result<NewRemoteServer, ApiError> {
    let input = validated_request(request)?;
    let cipher = provider_cipher(state)?;
    let private_key = input
        .private_key
        .ok_or(ApiError::BadRequest("SSH private key is required"))?;
    let public_key = input
        .public_key
        .ok_or(ApiError::BadRequest("SSH public key is required"))?;
    let known_hosts = input
        .known_hosts
        .ok_or(ApiError::BadRequest("known_hosts is required"))?;
    Ok(NewRemoteServer {
        name: input.name,
        host: input.host,
        port: input.port,
        username: input.username,
        deploy_path: input.deploy_path,
        private_key_ciphertext: cipher.encrypt(private_key.as_bytes())?,
        public_key_ciphertext: cipher.encrypt(public_key.as_bytes())?,
        known_hosts_ciphertext: cipher.encrypt(known_hosts.as_bytes())?,
        is_default: input.is_default,
    })
}

fn encrypt_update_request(
    state: &AppState,
    request: RemoteServerRequest,
) -> Result<RemoteServerUpdate, ApiError> {
    let input = validated_request(request)?;
    let cipher = provider_cipher(state)?;
    Ok(RemoteServerUpdate {
        name: input.name,
        host: input.host,
        port: input.port,
        username: input.username,
        deploy_path: input.deploy_path,
        private_key_ciphertext: input
            .private_key
            .map(|value| cipher.encrypt(value.as_bytes()))
            .transpose()?,
        public_key_ciphertext: input
            .public_key
            .map(|value| cipher.encrypt(value.as_bytes()))
            .transpose()?,
        known_hosts_ciphertext: input
            .known_hosts
            .map(|value| cipher.encrypt(value.as_bytes()))
            .transpose()?,
        is_default: input.is_default,
    })
}

#[derive(Debug)]
struct ValidatedRemoteServerRequest {
    name: String,
    host: String,
    port: u16,
    username: String,
    deploy_path: String,
    private_key: Option<String>,
    public_key: Option<String>,
    known_hosts: Option<String>,
    is_default: bool,
}

fn validated_request(
    request: RemoteServerRequest,
) -> Result<ValidatedRemoteServerRequest, ApiError> {
    Ok(ValidatedRemoteServerRequest {
        name: normalized_name(request.name)?,
        host: normalized_host(request.host)?,
        port: validated_port(request.port)?,
        username: normalized_username(request.username)?,
        deploy_path: normalized_deploy_path(request.deploy_path)?,
        private_key: optional_private_key(request.private_key)?,
        public_key: optional_public_key(request.public_key)?,
        known_hosts: optional_known_hosts(request.known_hosts)?,
        is_default: request.is_default,
    })
}

fn validated_port(value: u16) -> Result<u16, ApiError> {
    if value == 0 {
        return Err(ApiError::BadRequest("SSH port must be between 1 and 65535"));
    }
    Ok(value)
}

fn normalized_name(value: String) -> Result<String, ApiError> {
    let value = value.trim().to_owned();
    if value.is_empty() || value.chars().count() > 100 || value.chars().any(char::is_control) {
        return Err(ApiError::BadRequest(
            "remote server name must be 1-100 characters",
        ));
    }
    Ok(value)
}

fn normalized_host(value: String) -> Result<String, ApiError> {
    let value = value.trim().to_ascii_lowercase();
    let url = Url::parse(&format!("ssh://{value}"))
        .map_err(|_| ApiError::BadRequest("SSH host is invalid"))?;
    let host = url
        .host_str()
        .ok_or(ApiError::BadRequest("SSH host is invalid"))?;
    if url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(ApiError::BadRequest(
            "SSH host must not include a port or path",
        ));
    }
    Ok(host.to_owned())
}

fn normalized_username(value: String) -> Result<String, ApiError> {
    let value = value.trim().to_owned();
    let mut bytes = value.bytes();
    let starts_validly = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte == b'_');
    if value.is_empty()
        || value.len() > 32
        || !starts_validly
        || !bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
    {
        return Err(ApiError::BadRequest("SSH username is invalid"));
    }
    Ok(value)
}

fn normalized_deploy_path(value: String) -> Result<String, ApiError> {
    let value = value.trim().to_owned();
    if value.is_empty()
        || value.len() > 256
        || !value.starts_with('/')
        || value.contains('\u{5c}')
        || value.contains("//")
        || value.split('/').any(|segment| segment == "..")
        || value.chars().any(char::is_control)
    {
        return Err(ApiError::BadRequest(
            "deployment path must be an absolute Linux path",
        ));
    }
    let normalized = value.trim_end_matches('/');
    Ok(if normalized.is_empty() {
        "/".to_owned()
    } else {
        normalized.to_owned()
    })
}

fn optional_private_key(value: Option<String>) -> Result<Option<String>, ApiError> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(validate_private_key)
        .transpose()
}

fn validate_private_key(value: String) -> Result<String, ApiError> {
    if value.len() > MAX_SSH_SECRET_BYTES
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        || !value.contains("-----BEGIN")
        || !value.contains("PRIVATE KEY-----")
    {
        return Err(ApiError::BadRequest("SSH private key is invalid"));
    }
    Ok(value)
}

fn optional_public_key(value: Option<String>) -> Result<Option<String>, ApiError> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(validate_public_key)
        .transpose()
}

fn validate_public_key(value: String) -> Result<String, ApiError> {
    let mut fields = value.split_ascii_whitespace();
    let key_type = fields.next();
    let key = fields.next();
    let valid_type = key_type.is_some_and(|value| {
        value.starts_with("ssh-") || value.starts_with("ecdsa-") || value.starts_with("sk-")
    });
    let valid_key = key.is_some_and(|value| {
        (20..=16 * 1024).contains(&value.len())
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='))
    });
    if value.len() > MAX_SSH_SECRET_BYTES
        || value.chars().any(char::is_control)
        || !valid_type
        || !valid_key
    {
        return Err(ApiError::BadRequest("SSH public key is invalid"));
    }
    Ok(value)
}

fn optional_known_hosts(value: Option<String>) -> Result<Option<String>, ApiError> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(validate_known_hosts)
        .transpose()
}

fn validate_known_hosts(value: String) -> Result<String, ApiError> {
    let has_host_key = value.lines().any(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return false;
        }
        let mut fields = line.split_ascii_whitespace();
        let host = fields.next();
        let key_type = fields.next();
        let key = fields.next();
        host.is_some()
            && key.is_some()
            && key_type.is_some_and(|key_type| {
                key_type.starts_with("ssh-")
                    || key_type.starts_with("ecdsa-")
                    || key_type.starts_with("sk-")
            })
    });
    if value.len() > MAX_SSH_SECRET_BYTES
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        || !has_host_key
    {
        return Err(ApiError::BadRequest("known_hosts is invalid"));
    }
    Ok(value)
}

async fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    if require_actor(state, headers)
        .await?
        .has_platform_operator_access()
    {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

fn provider_cipher(state: &AppState) -> Result<&Arc<AgeCipher>, ApiError> {
    state
        .provider_cipher
        .as_ref()
        .ok_or(ApiError::ProviderCapabilityUnavailable)
}

fn wake_worker(state: &AppState) {
    if let Some(control) = &state.control {
        let _ = control.wake_worker();
    }
}
