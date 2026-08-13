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
use zeroize::Zeroizing;

use crate::{
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    handlers::remote_agent::RemoteServerAgentResponse,
    state::AppState,
};

mod onboarding;

use onboarding::{GeneratedKeyPair, generate_key_pair, scan_known_hosts};

const MAX_SSH_SECRET_BYTES: usize = 256 * 1024;
const DEFAULT_REMOTE_DEPLOY_PATH: &str = "/srv/ignitify";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateRemoteServerRequest {
    name: String,
    host: String,
    port: u16,
    username: String,
}

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
    agent: Option<RemoteServerAgentResponse>,
    is_default: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct RemoteServerCheckResponse {
    connected: bool,
    latency_ms: u64,
}

#[derive(Debug, Serialize)]
pub(crate) struct RemoteServerCreateResponse {
    #[serde(flatten)]
    server: RemoteServerResponse,
    public_key: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct RemoteServerAccessResponse {
    public_key: String,
}

impl RemoteServerResponse {
    fn from_record(value: RemoteServerRecord, agent: Option<RemoteServerAgentResponse>) -> Self {
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
            agent,
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
    let records = state.database.remote_servers().list().await?;
    let mut responses = Vec::with_capacity(records.len());
    for record in records {
        responses.push(remote_server_response(&state, record).await?);
    }
    Ok(Json(responses))
}

pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateRemoteServerRequest>,
) -> Result<(StatusCode, Json<RemoteServerCreateResponse>), ApiError> {
    require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let input = validated_create_request(request)?;
    let key_pair = generate_key_pair().await?;
    let known_hosts = scan_known_hosts(&input.host, input.port).await?;
    let is_default = state.database.remote_servers().list().await?.is_empty();
    let public_key = key_pair.public_key.clone();
    let record = state
        .database
        .remote_servers()
        .create(encrypt_generated_create_request(
            &state,
            input,
            key_pair,
            known_hosts,
            is_default,
        )?)
        .await?;
    wake_worker(&state);
    Ok((
        StatusCode::CREATED,
        Json(RemoteServerCreateResponse {
            server: remote_server_response(&state, record).await?,
            public_key,
        }),
    ))
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
    Ok(Json(remote_server_response(&state, record).await?))
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
    Ok(Json(remote_server_response(&state, record).await?))
}

async fn remote_server_response(
    state: &AppState,
    record: RemoteServerRecord,
) -> Result<RemoteServerResponse, ApiError> {
    let agent = state
        .database
        .remote_server_agents()
        .get(&record.id)
        .await?
        .map(Into::into);
    Ok(RemoteServerResponse::from_record(record, agent))
}

pub(crate) async fn access(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(server_id): Path<String>,
) -> Result<Json<RemoteServerAccessResponse>, ApiError> {
    require_admin(&state, &headers).await?;
    let connection = state
        .database
        .remote_servers()
        .connection(&server_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let cipher = provider_cipher(&state)?;
    let public_key = cipher
        .decrypt(&connection.public_key_ciphertext)
        .map_err(|_| ApiError::RemoteServerSetupFailed)?;
    let public_key =
        String::from_utf8(public_key.to_vec()).map_err(|_| ApiError::RemoteServerSetupFailed)?;
    let public_key =
        validate_public_key(public_key).map_err(|_| ApiError::RemoteServerSetupFailed)?;
    Ok(Json(RemoteServerAccessResponse { public_key }))
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
    let public_key = cipher
        .decrypt(&connection.public_key_ciphertext)
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
        write_private_key_file(&private_key_path, private_key.as_ref()).await?;
        write_secret_file(&known_hosts_path, known_hosts.as_ref()).await?;
        verify_key_pair(&private_key_path, public_key.as_ref()).await?;
        let started = Instant::now();
        let port = connection.port.to_string();
        let private_key_arg = private_key_path.to_string_lossy().into_owned();
        let known_hosts_option = format!("UserKnownHostsFile={}", known_hosts_path.display());
        let global_known_hosts_option = format!(
            "GlobalKnownHostsFile={}",
            if cfg!(windows) { "NUL" } else { "/dev/null" }
        );
        let target = format!("{}@{}", connection.username, connection.host);
        let output = timeout(
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
                .stderr(Stdio::piped())
                .output(),
        )
        .await
        .map_err(|_| {
            ApiError::RemoteServerCheckFailedWithReason(
                "SSH connection timed out. Verify the host, port, and firewall rule.",
            )
        })?
        .map_err(ssh_command_error)?;
        if !output.status.success() {
            let error = ssh_failure_error(&output.stderr);
            if is_authentication_failure(&output.stderr) {
                let _ = state
                    .database
                    .remote_server_agents()
                    .record_authentication_failure(&server_id)
                    .await;
            }
            return Err(error);
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

async fn write_private_key_file(path: &FsPath, contents: &[u8]) -> Result<(), ApiError> {
    let mut terminated = Zeroizing::new(contents.to_vec());
    if !terminated.ends_with(b"\n") {
        terminated.push(b'\n');
    }
    write_secret_file(path, terminated.as_ref()).await
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

async fn verify_key_pair(
    private_key_path: &FsPath,
    configured_public_key: &[u8],
) -> Result<(), ApiError> {
    let configured_public_key = String::from_utf8_lossy(configured_public_key);
    let Some(configured_material) = public_key_material(&configured_public_key) else {
        return Err(ApiError::RemoteServerCheckFailedWithReason(
            "SSH public key is missing or invalid. Update the server configuration.",
        ));
    };
    let private_key_arg = private_key_path.to_string_lossy().into_owned();
    let output = timeout(
        Duration::from_secs(5),
        Command::new("ssh-keygen")
            .kill_on_drop(true)
            .args(["-y", "-f", private_key_arg.as_str()])
            .env("LANG", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| {
        ApiError::RemoteServerCheckFailedWithReason(
            "SSH key verification timed out. Use a private key without a passphrase.",
        )
    })?
    .map_err(ssh_keygen_command_error)?;
    if !output.status.success() {
        let private_key = fs::read(private_key_path)
            .await
            .map_err(|_| ApiError::RemoteServerCheckFailed)?;
        return Err(ssh_keygen_failure_error(
            &output.stderr,
            private_key.as_ref(),
        ));
    }
    let derived_public_key = String::from_utf8_lossy(&output.stdout);
    if public_key_material(&derived_public_key) != Some(configured_material) {
        return Err(ApiError::RemoteServerCheckFailedWithReason(
            "Configured SSH private key does not match its public key. Update both fields with the same key pair.",
        ));
    }
    Ok(())
}

fn public_key_material(value: &str) -> Option<(&str, &str)> {
    let mut fields = value.split_ascii_whitespace();
    Some((fields.next()?, fields.next()?))
}

#[derive(Debug)]
struct ValidatedCreateRemoteServerRequest {
    name: String,
    host: String,
    port: u16,
    username: String,
}

fn validated_create_request(
    request: CreateRemoteServerRequest,
) -> Result<ValidatedCreateRemoteServerRequest, ApiError> {
    Ok(ValidatedCreateRemoteServerRequest {
        name: normalized_name(request.name)?,
        host: normalized_host(request.host)?,
        port: validated_port(request.port)?,
        username: normalized_username(request.username)?,
    })
}

fn encrypt_generated_create_request(
    state: &AppState,
    input: ValidatedCreateRemoteServerRequest,
    key_pair: GeneratedKeyPair,
    known_hosts: String,
    is_default: bool,
) -> Result<NewRemoteServer, ApiError> {
    let cipher = provider_cipher(state)?;
    Ok(NewRemoteServer {
        name: input.name,
        host: input.host,
        port: input.port,
        username: input.username,
        deploy_path: DEFAULT_REMOTE_DEPLOY_PATH.to_owned(),
        private_key_ciphertext: cipher.encrypt(key_pair.private_key.as_slice())?,
        public_key_ciphertext: cipher.encrypt(key_pair.public_key.as_bytes())?,
        known_hosts_ciphertext: cipher.encrypt(known_hosts.as_bytes())?,
        is_default,
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
        .map(normalize_private_key)
        .filter(|value| !value.is_empty())
        .map(validate_private_key)
        .transpose()
}

fn normalize_private_key(value: String) -> String {
    let mut lines = value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.len() < 3 {
        return value.trim().to_owned();
    }
    let header = lines.remove(0);
    let footer = lines.pop().unwrap_or_default();
    let body = lines.join("");
    format!("{header}\n{body}\n{footer}\n")
}

fn validate_private_key(value: String) -> Result<String, ApiError> {
    let non_empty_lines = value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let header = non_empty_lines.first().copied().unwrap_or_default();
    let footer = non_empty_lines.last().copied().unwrap_or_default();
    let expected_footer = header
        .strip_prefix("-----BEGIN ")
        .filter(|label| label.ends_with("PRIVATE KEY-----"))
        .map(|label| format!("-----END {label}"));
    let has_matching_footer = expected_footer.is_some_and(|expected| footer == expected);
    if value.len() > MAX_SSH_SECRET_BYTES
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        || !has_matching_footer
    {
        return Err(ApiError::BadRequest("SSH private key is invalid"));
    }
    Ok(value)
}

fn ssh_command_error(error: std::io::Error) -> ApiError {
    if error.kind() == std::io::ErrorKind::NotFound {
        return ApiError::RemoteServerCheckFailedWithReason(
            "SSH client is not installed on the Ignitify host.",
        );
    }
    ApiError::RemoteServerCheckFailedWithReason(
        "Unable to start the SSH connection check on the Ignitify host.",
    )
}

fn ssh_keygen_command_error(error: std::io::Error) -> ApiError {
    if error.kind() == std::io::ErrorKind::NotFound {
        return ApiError::RemoteServerCheckFailedWithReason(
            "SSH keygen utility is not installed on the Ignitify host.",
        );
    }
    ApiError::RemoteServerCheckFailedWithReason(
        "Unable to verify the configured SSH key pair on the Ignitify host.",
    )
}

fn ssh_keygen_failure_error(stderr: &[u8], private_key: &[u8]) -> ApiError {
    let stderr = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    let private_key_text = String::from_utf8_lossy(private_key);
    let line_count = private_key_text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    let message = if stderr.contains("passphrase")
        || stderr.contains("incorrect passphrase")
        || stderr.contains("decrypt")
    {
        "SSH private key has a passphrase. Generate a deployment key with an empty passphrase."
    } else if stderr.contains("invalid format") || stderr.contains("error in libcrypto") {
        "SSH private key format is invalid. Paste the complete key including its BEGIN and END lines."
    } else {
        "SSH private key could not be read. Verify that the pasted text is the original key file."
    };
    ApiError::RemoteServerCheckFailedWithDiagnostic(format!(
        "{message} Received {} bytes across {} non-empty lines; BEGIN marker: {}; END marker: {}.",
        private_key.len(),
        line_count,
        private_key
            .windows(b"-----BEGIN ".len())
            .any(|window| window == b"-----BEGIN "),
        private_key
            .windows(b"-----END ".len())
            .any(|window| window == b"-----END "),
    ))
}

fn ssh_failure_error(stderr: &[u8]) -> ApiError {
    let stderr = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    let message = if stderr.contains("could not resolve hostname") {
        "SSH hostname could not be resolved from the Ignitify host."
    } else if stderr.contains("connection refused") {
        "SSH port is closed or no SSH server is listening."
    } else if stderr.contains("connection timed out") || stderr.contains("operation timed out") {
        "SSH connection timed out. Verify the host, port, and firewall rule."
    } else if stderr.contains("no route to host") || stderr.contains("network is unreachable") {
        "The Ignitify host cannot reach this remote server."
    } else if stderr.contains("host key verification failed")
        || stderr.contains("no ed25519 host key is known")
        || stderr.contains("remote host identification has changed")
    {
        "SSH host key verification failed. Replace known_hosts with a verified current host key."
    } else if stderr.contains("no matching host key type found") {
        "The remote SSH server requires an unsupported host-key algorithm."
    } else if stderr.contains("permission denied") {
        "SSH authentication failed. Install the matching public key in ~/.ssh/authorized_keys."
    } else if stderr.contains("invalid format")
        || stderr.contains("error in libcrypto")
        || stderr.contains("load key")
    {
        "SSH private key is invalid. Upload the complete key including its BEGIN and END lines."
    } else {
        "SSH connection failed. Verify the host, port, SSH user, key pair, and known_hosts."
    };
    ApiError::RemoteServerCheckFailedWithReason(message)
}

fn is_authentication_failure(stderr: &[u8]) -> bool {
    String::from_utf8_lossy(stderr)
        .to_ascii_lowercase()
        .contains("permission denied")
}

fn optional_public_key(value: Option<String>) -> Result<Option<String>, ApiError> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(validate_public_key)
        .transpose()
}

pub(super) fn validate_public_key(value: String) -> Result<String, ApiError> {
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

pub(super) fn validate_known_hosts(value: String) -> Result<String, ApiError> {
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

#[cfg(test)]
mod tests;
