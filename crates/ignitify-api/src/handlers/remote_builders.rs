use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use ignitify_control_plane::AgeCipher;
use ignitify_db::{NewRemoteBuilder, RemoteBuilderRecord, RemoteBuilderUpdate};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

const MAX_PEM_BYTES: usize = 256 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoteBuilderRequest {
    name: String,
    endpoint: String,
    registry_repository: String,
    #[serde(default)]
    tls_server_name: Option<String>,
    ca_certificate: String,
    client_certificate: String,
    client_key: String,
    is_default: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct RemoteBuilderResponse {
    id: String,
    name: String,
    endpoint: String,
    registry_repository: String,
    tls_server_name: Option<String>,
    is_default: bool,
    created_at: String,
    updated_at: String,
}

impl From<RemoteBuilderRecord> for RemoteBuilderResponse {
    fn from(value: RemoteBuilderRecord) -> Self {
        Self {
            id: value.id,
            name: value.name,
            endpoint: value.endpoint,
            registry_repository: value.registry_repository,
            tls_server_name: value.tls_server_name,
            is_default: value.is_default,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<RemoteBuilderResponse>>, ApiError> {
    require_admin(&state, &headers).await?;
    let records = state
        .database
        .remote_builders()
        .list()
        .await?
        .into_iter()
        .map(RemoteBuilderResponse::from)
        .collect();
    Ok(Json(records))
}

pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RemoteBuilderRequest>,
) -> Result<(StatusCode, Json<RemoteBuilderResponse>), ApiError> {
    require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let record = state
        .database
        .remote_builders()
        .create(encrypt_request(&state, request)?)
        .await?;
    wake_worker(&state);
    Ok((StatusCode::CREATED, Json(record.into())))
}

pub(crate) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(builder_id): Path<String>,
    Json(request): Json<RemoteBuilderRequest>,
) -> Result<Json<RemoteBuilderResponse>, ApiError> {
    require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let input = encrypt_request(&state, request)?;
    let record = state
        .database
        .remote_builders()
        .update(
            &builder_id,
            RemoteBuilderUpdate {
                name: input.name,
                endpoint: input.endpoint,
                registry_repository: input.registry_repository,
                tls_server_name: input.tls_server_name,
                ca_certificate_ciphertext: input.ca_certificate_ciphertext,
                client_certificate_ciphertext: input.client_certificate_ciphertext,
                client_key_ciphertext: input.client_key_ciphertext,
                is_default: input.is_default,
            },
        )
        .await?
        .ok_or(ApiError::NotFound)?;
    wake_worker(&state);
    Ok(Json(record.into()))
}

pub(crate) async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(builder_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    if !state.database.remote_builders().delete(&builder_id).await? {
        return Err(ApiError::NotFound);
    }
    wake_worker(&state);
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn make_default(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(builder_id): Path<String>,
) -> Result<Json<RemoteBuilderResponse>, ApiError> {
    require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let record = state
        .database
        .remote_builders()
        .set_default(&builder_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    wake_worker(&state);
    Ok(Json(record.into()))
}

fn encrypt_request(
    state: &AppState,
    request: RemoteBuilderRequest,
) -> Result<NewRemoteBuilder, ApiError> {
    let name = normalized_name(request.name)?;
    let endpoint = normalized_endpoint(request.endpoint)?;
    let registry_repository = normalized_registry_repository(request.registry_repository)?;
    let tls_server_name = request
        .tls_server_name
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    if let Some(server_name) = tls_server_name.as_deref() {
        valid_hostname(server_name, "TLS server name")?;
    }
    let cipher = provider_cipher(state)?;
    Ok(NewRemoteBuilder {
        name,
        endpoint,
        registry_repository,
        tls_server_name,
        ca_certificate_ciphertext: cipher
            .encrypt(pem(request.ca_certificate, "CA certificate")?.as_bytes())?,
        client_certificate_ciphertext: cipher
            .encrypt(pem(request.client_certificate, "client certificate")?.as_bytes())?,
        client_key_ciphertext: cipher.encrypt(pem(request.client_key, "client key")?.as_bytes())?,
        is_default: request.is_default,
    })
}

fn normalized_name(value: String) -> Result<String, ApiError> {
    let value = value.trim().to_owned();
    if value.is_empty() || value.chars().count() > 100 || value.chars().any(char::is_control) {
        return Err(ApiError::BadRequest(
            "builder name must be 1-100 characters",
        ));
    }
    Ok(value)
}

fn normalized_endpoint(value: String) -> Result<String, ApiError> {
    let value = value.trim().to_owned();
    let url = Url::parse(&value)
        .map_err(|_| ApiError::BadRequest("remote builder endpoint is invalid"))?;
    if url.scheme() != "tcp"
        || url.host_str().is_none()
        || url.port().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(ApiError::BadRequest(
            "remote builder endpoint must be a tcp host and port",
        ));
    }
    Ok(value.trim_end_matches('/').to_owned())
}

fn normalized_registry_repository(value: String) -> Result<String, ApiError> {
    let value = value.trim().to_ascii_lowercase();
    let mut segments = value.split('/');
    let registry = segments.next().unwrap_or_default();
    let registry_is_valid = match registry.split_once(':') {
        Some((hostname, port)) => {
            !hostname.is_empty()
                && port.parse::<u16>().is_ok_and(|port| port != 0)
                && hostname.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'-')
                })
        }
        None => {
            !registry.is_empty()
                && registry.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'-')
                })
        }
    };
    if value.is_empty()
        || value.len() > 255
        || value.contains(char::is_whitespace)
        || value.contains('@')
        || value.starts_with(['/', '.'])
        || value.ends_with('/')
        || value.split('/').count() < 2
        || !registry_is_valid
        || segments.any(|segment| {
            segment.is_empty()
                || segment == "."
                || segment == ".."
                || !segment.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'_' | b'-')
                })
        })
    {
        return Err(ApiError::BadRequest(
            "registry repository must include a registry hostname and repository path",
        ));
    }
    Ok(value)
}

fn pem(value: String, label: &'static str) -> Result<String, ApiError> {
    let value = value.trim().to_owned();
    if value.is_empty()
        || value.len() > MAX_PEM_BYTES
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(ApiError::BadRequest("TLS material is invalid"));
    }
    let marker = if label == "client key" {
        "PRIVATE KEY"
    } else {
        "BEGIN CERTIFICATE"
    };
    if !value.contains(marker) {
        return Err(ApiError::BadRequest("TLS material is not PEM"));
    }
    Ok(value)
}

fn valid_hostname(value: &str, _label: &'static str) -> Result<(), ApiError> {
    let url = Url::parse(&format!("https://{value}"))
        .map_err(|_| ApiError::BadRequest("TLS server name is invalid"))?;
    if url.host_str().is_none() || url.port().is_some() || value.contains(['/', '@', ':']) {
        return Err(ApiError::BadRequest("TLS server name is invalid"));
    }
    Ok(())
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
