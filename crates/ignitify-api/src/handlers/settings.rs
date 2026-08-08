use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
};
use ignitify_control_plane::AgeCipher;
use ignitify_db::{
    NewServerCertificate, ServerCertificateRecord, ServerSettingsRecord, ServerSettingsUpdate,
};
use ignitify_domain::DomainName;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

const MAX_CERTIFICATE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ServerSettingsRequest {
    server_domain: String,
    https_enabled: bool,
    automatically_provision_ssl: bool,
    certificate_provider: String,
    custom_certificate_id: Option<String>,
    concurrent_builds: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct ServerSettingsResponse {
    server_domain: String,
    https_enabled: bool,
    automatically_provision_ssl: bool,
    certificate_provider: String,
    custom_certificate_id: Option<String>,
    concurrent_builds: i64,
    certificates: Vec<CertificateSummary>,
    updated_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct CertificateSummary {
    id: String,
    name: String,
    certificate_file_name: String,
    private_key_file_name: String,
    created_at: String,
    updated_at: String,
}

impl From<ServerCertificateRecord> for CertificateSummary {
    fn from(certificate: ServerCertificateRecord) -> Self {
        Self {
            id: certificate.id,
            name: certificate.name,
            certificate_file_name: certificate.certificate_file_name,
            private_key_file_name: certificate.private_key_file_name,
            created_at: certificate.created_at,
            updated_at: certificate.updated_at,
        }
    }
}

pub(crate) async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ServerSettingsResponse>, ApiError> {
    require_admin(&state, &headers).await?;
    response(&state).await.map(Json)
}

pub(crate) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ServerSettingsRequest>,
) -> Result<Json<ServerSettingsResponse>, ApiError> {
    require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let input = validate_request(&state, request).await?;
    state.database.server_settings().update(input).await?;
    wake_worker(&state);
    response(&state).await.map(Json)
}

pub(crate) async fn create_certificate(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<CertificateSummary>), ApiError> {
    require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;

    let mut name = None;
    let mut certificate = None;
    let mut private_key = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::BadRequest("invalid certificate upload"))?
    {
        match field.name() {
            Some("name") if name.is_none() => {
                name = Some(
                    field
                        .text()
                        .await
                        .map_err(|_| ApiError::BadRequest("invalid certificate name"))?,
                );
            }
            Some("certificate") if certificate.is_none() => {
                let file_name = safe_file_name(&field)?;
                let contents = read_certificate_field(field, "certificate").await?;
                certificate = Some((file_name, contents));
            }
            Some("private_key") if private_key.is_none() => {
                let file_name = safe_file_name(&field)?;
                let contents = read_certificate_field(field, "private key").await?;
                private_key = Some((file_name, contents));
            }
            _ => {}
        }
    }

    let name = name
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty() && value.chars().count() <= 100)
        .ok_or(ApiError::BadRequest(
            "certificate name must be 1-100 characters",
        ))?;
    let (certificate_file_name, certificate_contents) =
        certificate.ok_or(ApiError::BadRequest("certificate file is required"))?;
    let (private_key_file_name, private_key_contents) =
        private_key.ok_or(ApiError::BadRequest("private key file is required"))?;
    if !certificate_contents
        .windows(b"BEGIN CERTIFICATE".len())
        .any(|window| window == b"BEGIN CERTIFICATE")
    {
        return Err(ApiError::BadRequest(
            "certificate must be a PEM certificate",
        ));
    }
    if !private_key_contents
        .windows(b"PRIVATE KEY".len())
        .any(|window| window == b"PRIVATE KEY")
    {
        return Err(ApiError::BadRequest("private key must be a PEM key"));
    }

    let cipher = provider_cipher(&state)?;
    let certificate_ciphertext = cipher.encrypt(&certificate_contents)?;
    let private_key_ciphertext = cipher.encrypt(&private_key_contents)?;
    let record = state
        .database
        .server_settings()
        .create_certificate(NewServerCertificate {
            name,
            certificate_file_name,
            private_key_file_name,
            certificate_ciphertext,
            private_key_ciphertext,
        })
        .await?;
    wake_worker(&state);
    Ok((StatusCode::CREATED, Json(record.into())))
}

pub(crate) async fn remove_certificate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(certificate_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let repository = state.database.server_settings();
    if !repository.delete_certificate(&certificate_id).await? {
        return Err(ApiError::NotFound);
    }
    let current = repository.get().await?;
    if current.custom_certificate_id.as_deref() == Some(certificate_id.as_str()) {
        repository
            .update(ServerSettingsUpdate {
                server_domain: current.server_domain,
                https_enabled: current.https_enabled,
                automatically_provision_ssl: current.automatically_provision_ssl,
                certificate_provider: "none".to_owned(),
                custom_certificate_id: None,
                concurrent_builds: current.concurrent_builds,
            })
            .await?;
    }
    wake_worker(&state);
    Ok(StatusCode::NO_CONTENT)
}

async fn response(state: &AppState) -> Result<ServerSettingsResponse, ApiError> {
    let repository = state.database.server_settings();
    let settings = repository.get().await?;
    let certificates = repository
        .list_certificates()
        .await?
        .into_iter()
        .map(CertificateSummary::from)
        .collect();
    Ok(settings_response(settings, certificates))
}

fn settings_response(
    settings: ServerSettingsRecord,
    certificates: Vec<CertificateSummary>,
) -> ServerSettingsResponse {
    ServerSettingsResponse {
        server_domain: settings.server_domain,
        https_enabled: settings.https_enabled,
        automatically_provision_ssl: settings.automatically_provision_ssl,
        certificate_provider: settings.certificate_provider,
        custom_certificate_id: settings.custom_certificate_id,
        concurrent_builds: settings.concurrent_builds,
        certificates,
        updated_at: settings.updated_at,
    }
}

async fn validate_request(
    state: &AppState,
    request: ServerSettingsRequest,
) -> Result<ServerSettingsUpdate, ApiError> {
    let server_domain = request.server_domain.trim().to_ascii_lowercase();
    if server_domain.is_empty() {
        return Err(ApiError::BadRequest("server domain is required"));
    }
    DomainName::new(&server_domain).map_err(|_| {
        ApiError::BadRequest("server domain must be a valid hostname without a protocol or path")
    })?;
    if !(1..=32).contains(&request.concurrent_builds) {
        return Err(ApiError::BadRequest(
            "concurrent builds must be between 1 and 32",
        ));
    }

    let provider = request.certificate_provider.trim().to_ascii_lowercase();
    if !matches!(provider.as_str(), "none" | "lets-encrypt" | "custom") {
        return Err(ApiError::BadRequest("certificate provider is invalid"));
    }
    if request.https_enabled && request.automatically_provision_ssl && provider != "lets-encrypt" {
        return Err(ApiError::BadRequest(
            "automatic SSL provisioning requires Let's Encrypt",
        ));
    }
    let custom_certificate_id = if request.https_enabled && provider == "custom" {
        let id = request
            .custom_certificate_id
            .filter(|value| !value.trim().is_empty())
            .ok_or(ApiError::BadRequest("a custom certificate is required"))?;
        if !state
            .database
            .server_settings()
            .certificate_exists(&id)
            .await?
        {
            return Err(ApiError::BadRequest(
                "the selected custom certificate is unavailable",
            ));
        }
        Some(id)
    } else {
        None
    };

    Ok(ServerSettingsUpdate {
        server_domain,
        https_enabled: request.https_enabled,
        automatically_provision_ssl: request.https_enabled && request.automatically_provision_ssl,
        certificate_provider: if request.https_enabled {
            provider
        } else {
            "none".to_owned()
        },
        custom_certificate_id,
        concurrent_builds: request.concurrent_builds,
    })
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

fn wake_worker(state: &AppState) {
    if let Some(control) = &state.control {
        let _ = control.wake_worker();
    }
}

fn safe_file_name(field: &axum::extract::multipart::Field<'_>) -> Result<String, ApiError> {
    let file_name = field
        .file_name()
        .map(str::to_owned)
        .ok_or(ApiError::BadRequest("uploaded file must have a name"))?;
    if file_name.is_empty()
        || file_name.len() > 255
        || file_name.contains(['/', '\\'])
        || file_name.chars().any(char::is_control)
    {
        return Err(ApiError::BadRequest("uploaded file name is invalid"));
    }
    Ok(file_name)
}

async fn read_certificate_field(
    field: axum::extract::multipart::Field<'_>,
    label: &str,
) -> Result<Vec<u8>, ApiError> {
    let data = field
        .bytes()
        .await
        .map_err(|_| ApiError::BadRequest("invalid certificate upload"))?;
    if data.is_empty() || data.len() > MAX_CERTIFICATE_BYTES {
        return Err(ApiError::BadRequest(match label {
            "certificate" => "certificate file is empty or too large",
            _ => "private key file is empty or too large",
        }));
    }
    Ok(data.to_vec())
}
