use std::sync::Arc;

use axum::{
    Json,
    extract::{ConnectInfo, Extension, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Redirect,
};
use ignitify_auth::AuthenticatedUser;
use ignitify_control_plane::AgeCipher;
use ignitify_db::{
    AuditContext, AuditOutcome, NewProvider, ProviderAuthMode, ProviderKind,
    ProviderMutationOutcome, ProviderRecord, ProviderUpdate,
};
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    audit,
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::{AppState, GITHUB_MANIFEST_STATE_TTL},
};

#[derive(Debug, Deserialize)]
pub(crate) struct ProviderRequest {
    name: String,
    kind: String,
    auth_mode: Option<String>,
    base_url: String,
    internal_url: Option<String>,
    redirect_uri: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    application_id: Option<String>,
    installation_id: Option<String>,
    private_key: Option<String>,
    group_names: Option<String>,
    username: Option<String>,
    token: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ProviderResponse {
    id: String,
    name: String,
    kind: &'static str,
    auth_mode: &'static str,
    base_url: String,
    internal_url: Option<String>,
    redirect_uri: Option<String>,
    client_id: Option<String>,
    application_id: Option<String>,
    installation_id: Option<String>,
    group_names: Option<String>,
    username: Option<String>,
    token_configured: bool,
    created_at: String,
    updated_at: String,
    last_verified_at: Option<String>,
}

impl From<ProviderRecord> for ProviderResponse {
    fn from(provider: ProviderRecord) -> Self {
        Self {
            id: provider.id,
            name: provider.name,
            kind: provider.kind.as_str(),
            auth_mode: provider.auth_mode.as_str(),
            base_url: provider.base_url,
            internal_url: provider.internal_url,
            redirect_uri: provider.redirect_uri,
            client_id: provider.client_id,
            application_id: provider.application_id,
            installation_id: provider.installation_id,
            group_names: provider.group_names,
            username: provider.username,
            token_configured: !provider.credentials_ciphertext.is_empty(),
            created_at: provider.created_at,
            updated_at: provider.updated_at,
            last_verified_at: provider.last_verified_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct GithubManifestRequest {
    name: String,
    base_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct GithubManifestStartResponse {
    action_url: String,
    manifest: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GithubManifestCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubManifestConversion {
    id: u64,
    name: String,
    client_id: String,
    client_secret: String,
    pem: String,
}

pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProviderResponse>>, ApiError> {
    require_admin(&state, &headers).await?;
    let providers = state
        .database
        .providers()
        .list()
        .await?
        .into_iter()
        .map(ProviderResponse::from)
        .collect();
    Ok(Json(providers))
}

pub(crate) async fn start_github_manifest(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    Json(request): Json<GithubManifestRequest>,
) -> Result<Json<GithubManifestStartResponse>, ApiError> {
    let actor = require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;

    let requested_name = request.name.trim();
    if requested_name.is_empty() || requested_name.len() > 100 {
        return Err(ApiError::BadRequest(
            "GitHub App name must be 1-100 characters",
        ));
    }
    let name = github_manifest_name(requested_name);
    let base_url = request
        .base_url
        .as_deref()
        .unwrap_or("https://github.com")
        .trim()
        .trim_end_matches('/')
        .to_owned();
    let parsed_base_url =
        Url::parse(&base_url).map_err(|_| ApiError::BadRequest("provider URL is invalid"))?;
    if parsed_base_url.scheme() != "https" || parsed_base_url.host_str().is_none() {
        return Err(ApiError::BadRequest(
            "GitHub URL must use https and include a host",
        ));
    }
    if base_url != "https://github.com" {
        return Err(ApiError::BadRequest(
            "direct GitHub App connection currently supports https://github.com",
        ));
    }

    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .unwrap_or_else(|| frontend_origin(&state));
    let callback_url = format!(
        "{}/api/v1/providers/github/manifest/callback",
        origin.trim_end_matches('/')
    );
    let state_token = Uuid::new_v4().to_string();
    {
        let mut states = state.github_manifest_states.lock().await;
        states.retain(|_, pending| pending.created_at.elapsed() <= GITHUB_MANIFEST_STATE_TTL);
        states.insert(
            state_token.clone(),
            crate::state::GithubManifestPending {
                user_id: actor.id.clone(),
                name: name.clone(),
                base_url: base_url.clone(),
                frontend_origin: origin.clone(),
                created_at: std::time::Instant::now(),
            },
        );
    }

    audit::record(
        &state,
        Some(&actor),
        &headers,
        peer.as_deref(),
        "provider.github_manifest.start",
        Some("provider"),
        None,
        AuditOutcome::Success,
    )
    .await?;

    let manifest = serde_json::json!({
        "name": name,
        "url": origin,
        "redirect_url": callback_url,
        "public": false,
        "default_permissions": {
            "contents": "read",
            "metadata": "read"
        },
        "default_events": []
    });
    Ok(Json(GithubManifestStartResponse {
        action_url: format!("https://github.com/settings/apps/new?state={state_token}"),
        manifest,
    }))
}

pub(crate) async fn github_manifest_callback(
    State(state): State<AppState>,
    Query(query): Query<GithubManifestCallbackQuery>,
) -> Result<Redirect, ApiError> {
    let state_token = query
        .state
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or(ApiError::BadRequest("GitHub connection state is missing"))?;
    let pending = state
        .github_manifest_states
        .lock()
        .await
        .remove(state_token)
        .ok_or(ApiError::BadRequest(
            "GitHub connection state is invalid or expired",
        ))?;
    if pending.created_at.elapsed() > GITHUB_MANIFEST_STATE_TTL {
        return Err(ApiError::BadRequest(
            "GitHub connection state is invalid or expired",
        ));
    }

    let redirect_origin = pending.frontend_origin.clone();
    if query.error.is_some() {
        return Ok(Redirect::to(&github_result_redirect(
            &redirect_origin,
            "cancelled",
        )));
    }
    let code = query
        .code
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or(ApiError::BadRequest("GitHub authorization code is missing"))?;
    let conversion = exchange_github_manifest_code(code).await?;
    let credentials = ProviderCredentials {
        token: None,
        client_secret: Some(conversion.client_secret),
        private_key: Some(conversion.pem),
    };
    let provider_name = if conversion.name.trim().is_empty() {
        pending.name
    } else {
        conversion.name.trim().to_owned()
    };
    let input = NewProvider {
        name: provider_name,
        kind: ProviderKind::Github,
        auth_mode: ProviderAuthMode::GithubApp,
        base_url: pending.base_url,
        internal_url: None,
        redirect_uri: None,
        client_id: Some(conversion.client_id),
        application_id: Some(conversion.id.to_string()),
        installation_id: None,
        group_names: None,
        username: None,
        credentials_ciphertext: encrypt_credentials(&state, credentials)?,
    };
    let provider = state
        .database
        .providers()
        .create(&pending.user_id, input)
        .await?;
    state
        .database
        .users()
        .audit_event(
            Some(&pending.user_id),
            "provider.github_manifest.connect",
            Some("provider"),
            Some(&provider.id),
            &AuditContext::default(),
        )
        .await?;

    Ok(Redirect::to(&github_result_redirect(
        &redirect_origin,
        "connected",
    )))
}

async fn exchange_github_manifest_code(code: &str) -> Result<GithubManifestConversion, ApiError> {
    Ok(reqwest::Client::new()
        .post(format!(
            "https://api.github.com/app-manifests/{code}/conversions"
        ))
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header(reqwest::header::USER_AGENT, "Ignitify")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

fn frontend_origin(state: &AppState) -> String {
    state
        .origin_policy
        .public_origin()
        .unwrap_or_else(|| "http://localhost:6565".to_owned())
}

fn github_result_redirect(origin: &str, result: &str) -> String {
    format!("{}/providers?github={result}", origin.trim_end_matches('/'))
}

fn github_manifest_name(base_name: &str) -> String {
    let suffix = Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect::<String>();
    let max_base_length = 34usize.saturating_sub(suffix.len() + 1);
    let base = base_name
        .chars()
        .take(max_base_length)
        .collect::<String>()
        .trim_end_matches('-')
        .to_owned();
    if base.is_empty() {
        format!("Ignitify-{suffix}")
    } else {
        format!("{base}-{suffix}")
    }
}

pub(crate) async fn create(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    Json(request): Json<ProviderRequest>,
) -> Result<(StatusCode, Json<ProviderResponse>), ApiError> {
    let actor = require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let input = prepare_input(&state, request)?;
    let provider = state.database.providers().create(&actor.id, input).await?;
    audit::record(
        &state,
        Some(&actor),
        &headers,
        peer.as_deref(),
        "provider.create",
        Some("provider"),
        Some(&provider.id),
        AuditOutcome::Success,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(provider.into())))
}

pub(crate) async fn update(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    Path(provider_id): Path<String>,
    Json(request): Json<ProviderRequest>,
) -> Result<Json<ProviderResponse>, ApiError> {
    let actor = require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let input = prepare_update(&state, request)?;
    match state
        .database
        .providers()
        .update(&provider_id, input)
        .await?
    {
        ProviderMutationOutcome::Updated(provider) => {
            audit::record(
                &state,
                Some(&actor),
                &headers,
                peer.as_deref(),
                "provider.update",
                Some("provider"),
                Some(&provider_id),
                AuditOutcome::Success,
            )
            .await?;
            Ok(Json((*provider).into()))
        }
        ProviderMutationOutcome::Missing => Err(ApiError::NotFound),
    }
}

pub(crate) async fn remove(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    Path(provider_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let actor = require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    if state.database.providers().delete(&provider_id).await? {
        audit::record(
            &state,
            Some(&actor),
            &headers,
            peer.as_deref(),
            "provider.remove",
            Some("provider"),
            Some(&provider_id),
            AuditOutcome::Success,
        )
        .await?;
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}

async fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthenticatedUser, ApiError> {
    let actor = require_actor(state, headers).await?;
    if actor.has_platform_operator_access() {
        Ok(actor)
    } else {
        Err(ApiError::Forbidden)
    }
}

fn prepare_input(state: &AppState, request: ProviderRequest) -> Result<NewProvider, ApiError> {
    let kind = parse_kind(&request.kind)?;
    let auth_mode = parse_auth_mode(kind, request.auth_mode.as_deref())?;
    let metadata = validate_metadata(MetadataInput {
        name: request.name,
        base_url: request.base_url,
        internal_url: request.internal_url,
        redirect_uri: request.redirect_uri,
        client_id: request.client_id,
        application_id: request.application_id,
        installation_id: request.installation_id,
        group_names: request.group_names,
        username: request.username,
    })?;
    let credentials = ProviderCredentials {
        token: request.token,
        client_secret: request.client_secret,
        private_key: request.private_key,
    };
    validate_configuration(kind, auth_mode, &metadata, &credentials)?;
    Ok(NewProvider {
        name: metadata.name,
        kind,
        auth_mode,
        base_url: metadata.base_url,
        internal_url: metadata.internal_url,
        redirect_uri: metadata.redirect_uri,
        client_id: metadata.client_id,
        application_id: metadata.application_id,
        installation_id: metadata.installation_id,
        group_names: metadata.group_names,
        username: metadata.username,
        credentials_ciphertext: encrypt_credentials(state, credentials)?,
    })
}

fn prepare_update(state: &AppState, request: ProviderRequest) -> Result<ProviderUpdate, ApiError> {
    let kind = parse_kind(&request.kind)?;
    let auth_mode = parse_auth_mode(kind, request.auth_mode.as_deref())?;
    let metadata = validate_metadata(MetadataInput {
        name: request.name,
        base_url: request.base_url,
        internal_url: request.internal_url,
        redirect_uri: request.redirect_uri,
        client_id: request.client_id,
        application_id: request.application_id,
        installation_id: request.installation_id,
        group_names: request.group_names,
        username: request.username,
    })?;
    let credentials = ProviderCredentials {
        token: request.token,
        client_secret: request.client_secret,
        private_key: request.private_key,
    };
    validate_configuration(kind, auth_mode, &metadata, &credentials)?;
    Ok(ProviderUpdate {
        name: metadata.name,
        kind,
        auth_mode,
        base_url: metadata.base_url,
        internal_url: metadata.internal_url,
        redirect_uri: metadata.redirect_uri,
        client_id: metadata.client_id,
        application_id: metadata.application_id,
        installation_id: metadata.installation_id,
        group_names: metadata.group_names,
        username: metadata.username,
        credentials_ciphertext: encrypt_credentials_if_present(state, credentials)?,
    })
}

#[derive(Debug, Serialize)]
struct ProviderCredentials {
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    private_key: Option<String>,
}

#[derive(Debug)]
struct ProviderMetadata {
    name: String,
    base_url: String,
    internal_url: Option<String>,
    redirect_uri: Option<String>,
    client_id: Option<String>,
    application_id: Option<String>,
    installation_id: Option<String>,
    group_names: Option<String>,
    username: Option<String>,
}

#[derive(Debug)]
struct MetadataInput {
    name: String,
    base_url: String,
    internal_url: Option<String>,
    redirect_uri: Option<String>,
    client_id: Option<String>,
    application_id: Option<String>,
    installation_id: Option<String>,
    group_names: Option<String>,
    username: Option<String>,
}

fn encrypt_credentials(
    state: &AppState,
    credentials: ProviderCredentials,
) -> Result<String, ApiError> {
    if credentials.token.is_none()
        && credentials.client_secret.is_none()
        && credentials.private_key.is_none()
    {
        return Err(ApiError::BadRequest("provider credentials are required"));
    }
    encrypt_credentials_payload(state, credentials)
}

fn encrypt_credentials_if_present(
    state: &AppState,
    credentials: ProviderCredentials,
) -> Result<Option<String>, ApiError> {
    if credentials.token.is_none()
        && credentials.client_secret.is_none()
        && credentials.private_key.is_none()
    {
        return Ok(None);
    }
    encrypt_credentials_payload(state, credentials).map(Some)
}

fn encrypt_credentials_payload(
    state: &AppState,
    credentials: ProviderCredentials,
) -> Result<String, ApiError> {
    let plaintext = Zeroizing::new(
        serde_json::to_vec(&credentials)
            .map_err(|_| ApiError::BadRequest("invalid provider credentials"))?,
    );
    if plaintext.len() > 32_768 {
        return Err(ApiError::BadRequest("provider credentials are too long"));
    }
    Ok(provider_cipher(state)?.encrypt(plaintext.as_slice())?)
}

fn provider_cipher(state: &AppState) -> Result<&Arc<AgeCipher>, ApiError> {
    state
        .provider_cipher
        .as_ref()
        .ok_or(ApiError::ProviderCapabilityUnavailable)
}

fn parse_kind(kind: &str) -> Result<ProviderKind, ApiError> {
    match kind.trim().to_ascii_lowercase().as_str() {
        "git" => Ok(ProviderKind::Git),
        "gitea" => Ok(ProviderKind::Gitea),
        "gitlab" => Ok(ProviderKind::Gitlab),
        "github" => Ok(ProviderKind::Github),
        _ => Err(ApiError::BadRequest(
            "provider kind must be git, gitea, gitlab, or github",
        )),
    }
}

fn parse_auth_mode(
    kind: ProviderKind,
    auth_mode: Option<&str>,
) -> Result<ProviderAuthMode, ApiError> {
    let mode = match auth_mode.unwrap_or(match kind {
        ProviderKind::Git => "token",
        ProviderKind::Github | ProviderKind::Gitea | ProviderKind::Gitlab => "oauth",
    }) {
        "token" => ProviderAuthMode::Token,
        "oauth" => ProviderAuthMode::OAuth,
        "github_app" => ProviderAuthMode::GithubApp,
        _ => return Err(ApiError::BadRequest("provider auth mode is invalid")),
    };
    if kind != ProviderKind::Github && mode == ProviderAuthMode::GithubApp {
        return Err(ApiError::BadRequest(
            "GitHub App mode is only available for GitHub",
        ));
    }
    if kind != ProviderKind::Git && kind != ProviderKind::Github && mode == ProviderAuthMode::Token
    {
        return Err(ApiError::BadRequest(
            "this provider requires OAuth credentials",
        ));
    }
    Ok(mode)
}

fn validate_metadata(input: MetadataInput) -> Result<ProviderMetadata, ApiError> {
    let MetadataInput {
        name,
        base_url,
        internal_url,
        redirect_uri,
        client_id,
        application_id,
        installation_id,
        group_names,
        username,
    } = input;
    let name = name.trim().to_owned();
    if name.is_empty() || name.len() > 100 {
        return Err(ApiError::BadRequest(
            "provider name must be 1-100 characters",
        ));
    }
    let base_url = base_url.trim().trim_end_matches('/').to_owned();
    let parsed =
        Url::parse(&base_url).map_err(|_| ApiError::BadRequest("provider URL is invalid"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(ApiError::BadRequest("provider URL must use http or https"));
    }
    let internal_url = validate_optional_url(internal_url, "internal URL")?;
    let redirect_uri = validate_optional_url(redirect_uri, "redirect URI")?;
    let client_id = normalize_field(client_id, "client ID", 200)?;
    let application_id = normalize_field(application_id, "application ID", 100)?;
    let installation_id = normalize_field(installation_id, "installation ID", 100)?;
    let group_names = normalize_field(group_names, "group names", 1000)?;
    let username = username
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if username.as_ref().is_some_and(|value| value.len() > 100) {
        return Err(ApiError::BadRequest("provider username is too long"));
    }
    Ok(ProviderMetadata {
        name,
        base_url,
        internal_url,
        redirect_uri,
        client_id,
        application_id,
        installation_id,
        group_names,
        username,
    })
}

fn validate_optional_url(value: Option<String>, label: &str) -> Result<Option<String>, ApiError> {
    let Some(value) = value.map(|value| value.trim().trim_end_matches('/').to_owned()) else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }
    let parsed = Url::parse(&value).map_err(|_| ApiError::BadRequest("provider URL is invalid"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(ApiError::BadRequest(match label {
            "redirect URI" => "redirect URI must use http or https",
            _ => "internal URL must use http or https",
        }));
    }
    Ok(Some(value))
}

fn normalize_field(
    value: Option<String>,
    label: &str,
    max_length: usize,
) -> Result<Option<String>, ApiError> {
    let value = value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if value.as_ref().is_some_and(|value| value.len() > max_length) {
        return Err(ApiError::BadRequest(match label {
            "group names" => "group names are too long",
            "application ID" => "application ID is too long",
            "installation ID" => "installation ID is too long",
            _ => "provider field is too long",
        }));
    }
    Ok(value)
}

fn validate_configuration(
    kind: ProviderKind,
    mode: ProviderAuthMode,
    metadata: &ProviderMetadata,
    credentials: &ProviderCredentials,
) -> Result<(), ApiError> {
    match mode {
        ProviderAuthMode::Token => {
            if credentials
                .token
                .as_ref()
                .is_none_or(|token| token.trim().is_empty())
            {
                return Err(ApiError::BadRequest("provider token is required"));
            }
        }
        ProviderAuthMode::OAuth => {
            if metadata.redirect_uri.is_none() || metadata.client_id.is_none() {
                return Err(ApiError::BadRequest(
                    "redirect URI and client ID are required",
                ));
            }
            if credentials
                .client_secret
                .as_ref()
                .is_none_or(|secret| secret.trim().is_empty())
            {
                return Err(ApiError::BadRequest("client secret is required"));
            }
        }
        ProviderAuthMode::GithubApp => {
            if kind != ProviderKind::Github {
                return Err(ApiError::BadRequest(
                    "GitHub App mode is only available for GitHub",
                ));
            }
            if metadata.application_id.is_none() {
                return Err(ApiError::BadRequest("GitHub App ID is required"));
            }
            if credentials
                .private_key
                .as_ref()
                .is_none_or(|key| key.trim().is_empty())
            {
                return Err(ApiError::BadRequest("GitHub App private key is required"));
            }
        }
    }
    Ok(())
}
