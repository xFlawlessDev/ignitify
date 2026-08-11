use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
};
use ignitify_control_plane::AgeCipher;
use ignitify_db::{
    NewServerCertificate, ServerCertificateRecord, ServerSettingsRecord, ServerSettingsUpdate,
};
use ignitify_domain::{DnsRecord, DnsRecordType, DomainName};
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
pub(crate) struct InfrastructureSettingsRequest {
    #[serde(default)]
    control_plane_domain: String,
    application_domain_suffix: String,
    https_enabled: bool,
    automatically_provision_ssl: bool,
    acme_email: String,
    #[serde(default = "default_dns_record_type")]
    dns_record_type: String,
    #[serde(default)]
    dns_record_target: String,
    #[serde(default)]
    fallback_page_heading: Option<String>,
    #[serde(default)]
    fallback_page_message: Option<String>,
    certificate_provider: String,
    custom_certificate_id: Option<String>,
    #[serde(default)]
    concurrent_builds: Option<i64>,
}

fn default_dns_record_type() -> String {
    "a".to_owned()
}

#[derive(Debug, Serialize)]
pub(crate) struct InfrastructureSettingsResponse {
    application: ApplicationEnvironmentResponse,
    control_plane_domain: String,
    application_domain_suffix: String,
    https_enabled: bool,
    automatically_provision_ssl: bool,
    acme_email: String,
    dns_record_type: String,
    dns_record_target: String,
    fallback_page_heading: String,
    fallback_page_message: String,
    certificate_provider: String,
    custom_certificate_id: Option<String>,
    concurrent_builds: i64,
    certificates: Vec<CertificateSummary>,
    health: InfrastructureHealthResponse,
    updated_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ApplicationEnvironmentResponse {
    public_origin: String,
    secure_cookies: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct InfrastructureHealthResponse {
    database: &'static str,
    runtime: &'static str,
    worker: &'static str,
    ingress: &'static str,
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
) -> Result<Json<InfrastructureSettingsResponse>, ApiError> {
    require_admin(&state, &headers).await?;
    response(&state).await.map(Json)
}

pub(crate) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<InfrastructureSettingsRequest>,
) -> Result<Json<InfrastructureSettingsResponse>, ApiError> {
    require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let input = validate_request(&state, request).await?;
    let settings = state.database.server_settings().update(input).await?;
    if !state.origin_policy.set_control_plane_domain(
        (!settings.control_plane_domain.is_empty())
            .then_some(settings.control_plane_domain.clone()),
    ) {
        return Err(ApiError::RuntimeStateUnavailable);
    }
    wake_worker(&state);
    response_from_settings(&state, settings).await.map(Json)
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
                application_domain_suffix: current.application_domain_suffix,
                https_enabled: current.https_enabled,
                automatically_provision_ssl: current.automatically_provision_ssl,
                acme_email: current.acme_email,
                dns_record_type: current.dns_record_type,
                dns_record_target: current.dns_record_target,
                fallback_page_heading: current.fallback_page_heading,
                fallback_page_message: current.fallback_page_message,
                certificate_provider: "none".to_owned(),
                custom_certificate_id: None,
                control_plane_domain: current.control_plane_domain,
                concurrent_builds: current.concurrent_builds,
            })
            .await?;
    }
    wake_worker(&state);
    Ok(StatusCode::NO_CONTENT)
}

async fn response(state: &AppState) -> Result<InfrastructureSettingsResponse, ApiError> {
    let repository = state.database.server_settings();
    let settings = repository.get().await?;
    response_from_settings(state, settings).await
}

async fn response_from_settings(
    state: &AppState,
    settings: ServerSettingsRecord,
) -> Result<InfrastructureSettingsResponse, ApiError> {
    let repository = state.database.server_settings();
    let certificates = repository
        .list_certificates()
        .await?
        .into_iter()
        .map(CertificateSummary::from)
        .collect();
    let (database, runtime, worker, ingress) = tokio::join!(
        state.database.ping(),
        state.runtime_health.ready(),
        state.worker_health.ready(),
        state.ingress_health.ready(),
    );
    Ok(settings_response(
        settings,
        certificates,
        ApplicationEnvironmentResponse {
            public_origin: state.origin_policy.public_origin().unwrap_or_default(),
            secure_cookies: state.secure_cookies,
        },
        InfrastructureHealthResponse {
            database: if database.is_ok() {
                "ready"
            } else {
                "unavailable"
            },
            runtime: if runtime { "ready" } else { "unavailable" },
            worker: if worker { "ready" } else { "unavailable" },
            ingress: if ingress { "ready" } else { "unavailable" },
        },
    ))
}

fn settings_response(
    settings: ServerSettingsRecord,
    certificates: Vec<CertificateSummary>,
    application: ApplicationEnvironmentResponse,
    health: InfrastructureHealthResponse,
) -> InfrastructureSettingsResponse {
    InfrastructureSettingsResponse {
        application,
        control_plane_domain: settings.control_plane_domain,
        application_domain_suffix: settings.application_domain_suffix,
        https_enabled: settings.https_enabled,
        automatically_provision_ssl: settings.automatically_provision_ssl,
        acme_email: settings.acme_email,
        dns_record_type: settings.dns_record_type,
        dns_record_target: settings.dns_record_target,
        fallback_page_heading: settings.fallback_page_heading,
        fallback_page_message: settings.fallback_page_message,
        certificate_provider: settings.certificate_provider,
        custom_certificate_id: settings.custom_certificate_id,
        concurrent_builds: settings.concurrent_builds,
        certificates,
        health,
        updated_at: settings.updated_at,
    }
}

async fn validate_request(
    state: &AppState,
    request: InfrastructureSettingsRequest,
) -> Result<ServerSettingsUpdate, ApiError> {
    let application_domain_suffix = request
        .application_domain_suffix
        .trim()
        .to_ascii_lowercase();
    if application_domain_suffix.is_empty() {
        return Err(ApiError::BadRequest(
            "application domain suffix is required",
        ));
    }
    DomainName::new(&application_domain_suffix).map_err(|_| {
        ApiError::BadRequest(
            "application domain suffix must be a valid hostname without a protocol or path",
        )
    })?;
    let control_plane_domain = request.control_plane_domain.trim().to_ascii_lowercase();
    if !control_plane_domain.is_empty() {
        DomainName::new(&control_plane_domain).map_err(|_| {
            ApiError::BadRequest(
                "control plane domain must be a valid hostname without a protocol or path",
            )
        })?;
        if control_plane_domain == application_domain_suffix
            || control_plane_domain.ends_with(&format!(".{application_domain_suffix}"))
        {
            return Err(ApiError::BadRequest(
                "control plane domain must be separate from the managed application suffix",
            ));
        }
        if !state.secure_cookies {
            return Err(ApiError::BadRequest(
                "control plane domain requires secure cookies",
            ));
        }
    }
    let current = state.database.server_settings().get().await?;
    let concurrent_builds = request
        .concurrent_builds
        .unwrap_or(current.concurrent_builds);
    if !(1..=32).contains(&concurrent_builds) {
        return Err(ApiError::BadRequest(
            "concurrent builds must be between 1 and 32",
        ));
    }
    let fallback_page_heading = normalized_fallback_content(
        request
            .fallback_page_heading
            .unwrap_or(current.fallback_page_heading),
        "fallback page heading",
        100,
    )?;
    let fallback_page_message = normalized_fallback_content(
        request
            .fallback_page_message
            .unwrap_or(current.fallback_page_message),
        "fallback page message",
        280,
    )?;

    let provider = request.certificate_provider.trim().to_ascii_lowercase();
    if !matches!(provider.as_str(), "none" | "lets-encrypt" | "custom") {
        return Err(ApiError::BadRequest("certificate provider is invalid"));
    }
    if request.https_enabled && request.automatically_provision_ssl && provider != "lets-encrypt" {
        return Err(ApiError::BadRequest(
            "automatic SSL provisioning requires Let's Encrypt",
        ));
    }
    let acme_email = request.acme_email.trim().to_owned();
    if request.https_enabled && request.automatically_provision_ssl && !valid_email(&acme_email) {
        return Err(ApiError::BadRequest(
            "a valid ACME contact email is required for automatic certificates",
        ));
    }
    let dns_record_type = request.dns_record_type.trim().to_ascii_lowercase();
    let dns_record_target = request.dns_record_target.trim().to_ascii_lowercase();
    let dns_record_kind = DnsRecordType::try_from(dns_record_type.as_str())
        .map_err(|_| ApiError::BadRequest("DNS record type must be A or CNAME"))?;
    if !dns_record_target.is_empty() {
        DnsRecord::new(dns_record_kind, &dns_record_target)
            .map_err(|_| ApiError::BadRequest("DNS record target is invalid"))?;
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
    if !control_plane_domain.is_empty() && !request.https_enabled {
        return Err(ApiError::BadRequest(
            "control plane domain requires HTTPS ingress",
        ));
    }
    if !control_plane_domain.is_empty()
        && provider != "custom"
        && !(request.automatically_provision_ssl && provider == "lets-encrypt")
    {
        return Err(ApiError::BadRequest(
            "control plane domain requires automatic or custom TLS certificates",
        ));
    }

    Ok(ServerSettingsUpdate {
        control_plane_domain,
        application_domain_suffix,
        https_enabled: request.https_enabled,
        automatically_provision_ssl: request.https_enabled && request.automatically_provision_ssl,
        acme_email,
        dns_record_type,
        dns_record_target,
        fallback_page_heading,
        fallback_page_message,
        certificate_provider: if request.https_enabled {
            provider
        } else {
            "none".to_owned()
        },
        custom_certificate_id,
        concurrent_builds,
    })
}

fn normalized_fallback_content(
    value: String,
    field: &'static str,
    maximum: usize,
) -> Result<String, ApiError> {
    let value = value.replace("\r\n", "\n").replace('\r', "\n");
    let value = value.trim().to_owned();
    let allows_newlines = field == "fallback page message";
    if value.is_empty()
        || value.chars().count() > maximum
        || value
            .chars()
            .any(|character| character.is_control() && !(allows_newlines && character == '\n'))
    {
        return Err(ApiError::BadRequest(match field {
            "fallback page heading" => "fallback page heading must be 1-100 characters",
            _ => "fallback page message must be 1-280 characters",
        }));
    }
    Ok(value)
}

fn valid_email(value: &str) -> bool {
    if value.is_empty() || value.len() > 254 || value.chars().any(char::is_whitespace) {
        return false;
    }
    let mut parts = value.split('@');
    let Some(local) = parts.next() else {
        return false;
    };
    let Some(domain) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && !local.is_empty()
        && !domain.is_empty()
        && DomainName::new(domain).is_ok()
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
