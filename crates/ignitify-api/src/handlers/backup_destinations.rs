use std::sync::Arc;

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use ignitify_control_plane::AgeCipher;
use ignitify_db::{BackupS3DestinationRecord, NewBackupS3Destination};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

const MAX_SESSION_TOKEN_BYTES: usize = 4 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BackupS3DestinationRequest {
    endpoint: String,
    region: String,
    bucket: String,
    #[serde(default)]
    prefix: String,
    access_key_id: String,
    secret_access_key: String,
    #[serde(default)]
    session_token: Option<String>,
    #[serde(default = "default_server_side_encryption")]
    server_side_encryption: String,
}

fn default_server_side_encryption() -> String {
    "AES256".to_owned()
}

#[derive(Debug, Serialize)]
pub(crate) struct BackupS3DestinationResponse {
    endpoint: String,
    region: String,
    bucket: String,
    prefix: String,
    server_side_encryption: String,
    created_at: String,
    updated_at: String,
}

impl From<BackupS3DestinationRecord> for BackupS3DestinationResponse {
    fn from(value: BackupS3DestinationRecord) -> Self {
        Self {
            endpoint: value.endpoint,
            region: value.region,
            bucket: value.bucket,
            prefix: value.prefix,
            server_side_encryption: value.server_side_encryption,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

pub(crate) async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Option<BackupS3DestinationResponse>>, ApiError> {
    require_admin(&state, &headers).await?;
    let destination = state
        .database
        .backup_destinations()
        .s3()
        .await?
        .map(BackupS3DestinationResponse::from);
    Ok(Json(destination))
}

pub(crate) async fn upsert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BackupS3DestinationRequest>,
) -> Result<Json<BackupS3DestinationResponse>, ApiError> {
    require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let record = state
        .database
        .backup_destinations()
        .upsert_s3(encrypt_request(&state, request)?)
        .await?;
    Ok(Json(record.into()))
}

pub(crate) async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    state.database.backup_destinations().delete_s3().await?;
    Ok(StatusCode::NO_CONTENT)
}

fn encrypt_request(
    state: &AppState,
    request: BackupS3DestinationRequest,
) -> Result<NewBackupS3Destination, ApiError> {
    let endpoint = normalized_endpoint(request.endpoint)?;
    let region = normalized_region(request.region)?;
    let bucket = normalized_bucket(request.bucket)?;
    let prefix = normalized_prefix(request.prefix)?;
    let access_key_id = credential(request.access_key_id, "access key ID", 128)?;
    let secret_access_key = credential(request.secret_access_key, "secret access key", 256)?;
    let session_token = request
        .session_token
        .map(|value| credential(value, "session token", MAX_SESSION_TOKEN_BYTES))
        .transpose()?;
    let server_side_encryption = normalized_server_side_encryption(request.server_side_encryption)?;
    let cipher = provider_cipher(state)?;
    Ok(NewBackupS3Destination {
        endpoint,
        region,
        bucket,
        prefix,
        access_key_id_ciphertext: cipher.encrypt(access_key_id.as_bytes())?,
        secret_access_key_ciphertext: cipher.encrypt(secret_access_key.as_bytes())?,
        session_token_ciphertext: session_token
            .as_deref()
            .map(|value| cipher.encrypt(value.as_bytes()))
            .transpose()?,
        server_side_encryption,
    })
}

fn normalized_endpoint(value: String) -> Result<String, ApiError> {
    let value = value.trim().trim_end_matches('/').to_owned();
    let url = Url::parse(&value).map_err(|_| ApiError::BadRequest("S3 endpoint is invalid"))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err(ApiError::BadRequest(
            "S3 endpoint must be an HTTPS origin without a path",
        ));
    }
    Ok(value)
}

fn normalized_region(value: String) -> Result<String, ApiError> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(ApiError::BadRequest("S3 region is invalid"));
    }
    Ok(value)
}

fn normalized_bucket(value: String) -> Result<String, ApiError> {
    let value = value.trim().to_ascii_lowercase();
    if !(3..=63).contains(&value.len())
        || value.starts_with(['.', '-'])
        || value.ends_with(['.', '-'])
        || value.contains("..")
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
    {
        return Err(ApiError::BadRequest("S3 bucket is invalid"));
    }
    Ok(value)
}

fn normalized_prefix(value: String) -> Result<String, ApiError> {
    let value = value.trim().trim_matches('/').to_owned();
    if (!value.is_empty()
        && value.split('/').any(|segment| {
            segment.is_empty()
                || matches!(segment, "." | "..")
                || !segment.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'_' | b'-')
                })
        }))
        || value.len() > 512
    {
        return Err(ApiError::BadRequest("S3 backup prefix is invalid"));
    }
    Ok(value)
}

fn normalized_server_side_encryption(value: String) -> Result<String, ApiError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "provider-default" => Ok("provider-default".to_owned()),
        "aes256" => Ok("AES256".to_owned()),
        _ => Err(ApiError::BadRequest("S3 server-side encryption is invalid")),
    }
}

fn credential(value: String, label: &'static str, maximum: usize) -> Result<String, ApiError> {
    let value = value.trim().to_owned();
    if value.is_empty()
        || value.len() > maximum
        || value.chars().any(char::is_control)
        || value.chars().any(char::is_whitespace)
    {
        return Err(ApiError::BadRequest(match label {
            "access key ID" => "S3 access key ID is invalid",
            "secret access key" => "S3 secret access key is invalid",
            _ => "S3 session token is invalid",
        }));
    }
    Ok(value)
}

async fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    if require_actor(state, headers).await?.has_admin_access() {
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
