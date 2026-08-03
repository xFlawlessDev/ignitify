use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use ignitify_db::{NewWebhook, WebhookActor, WebhookMutationOutcome, WebhookRecord};
use serde::{Deserialize, Serialize};
use url::Url;
use zeroize::Zeroizing;

use crate::{
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WebhookRequest {
    name: String,
    url: String,
    secret: Option<String>,
    is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoveWebhookRequest {
    confirm_name: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct WebhookResponse {
    id: String,
    project_id: String,
    name: String,
    url: String,
    secret_configured: bool,
    is_enabled: bool,
    created_at: String,
    updated_at: String,
}

impl From<WebhookRecord> for WebhookResponse {
    fn from(record: WebhookRecord) -> Self {
        Self {
            id: record.id,
            project_id: record.project_id,
            name: record.name,
            url: record.url,
            secret_configured: record.secret_ciphertext.is_some(),
            is_enabled: record.is_enabled,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
) -> Result<Json<Vec<WebhookResponse>>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let webhooks = state
        .database
        .webhooks()
        .list(webhook_actor(&actor), &project_id)
        .await?
        .ok_or(ApiError::NotFound)?
        .into_iter()
        .map(WebhookResponse::from)
        .collect();
    Ok(Json(webhooks))
}

pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(request): Json<WebhookRequest>,
) -> Result<(StatusCode, Json<WebhookResponse>), ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let outcome = state
        .database
        .webhooks()
        .create(webhook_actor(&actor), &project_id, input(&state, request)?)
        .await?;
    let record = match outcome {
        WebhookMutationOutcome::Created(record) => record,
        WebhookMutationOutcome::Missing => return Err(ApiError::NotFound),
        WebhookMutationOutcome::Forbidden => return Err(ApiError::Forbidden),
        WebhookMutationOutcome::Removed(_) => {
            return Err(ApiError::BadRequest("invalid webhook operation"));
        }
    };
    Ok((StatusCode::CREATED, Json(record.into())))
}

pub(crate) async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(webhook_id): Path<String>,
    Json(request): Json<RemoveWebhookRequest>,
) -> Result<Json<WebhookResponse>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let outcome = state
        .database
        .webhooks()
        .remove(webhook_actor(&actor), &webhook_id, &request.confirm_name)
        .await?;
    let record = match outcome {
        WebhookMutationOutcome::Removed(record) => record,
        WebhookMutationOutcome::Missing => return Err(ApiError::NotFound),
        WebhookMutationOutcome::Forbidden => return Err(ApiError::Forbidden),
        WebhookMutationOutcome::Created(_) => {
            return Err(ApiError::BadRequest("invalid webhook operation"));
        }
    };
    Ok(Json(record.into()))
}

fn webhook_actor(actor: &ignitify_auth::AuthenticatedUser) -> WebhookActor<'_> {
    WebhookActor {
        id: &actor.id,
        is_admin: actor.has_admin_access(),
    }
}

fn input(state: &AppState, request: WebhookRequest) -> Result<NewWebhook, ApiError> {
    let name = request.name.trim();
    if name.is_empty() || name.len() > 100 || name.chars().any(char::is_control) {
        return Err(ApiError::BadRequest("invalid webhook name"));
    }
    let url = Url::parse(&request.url).map_err(|_| ApiError::BadRequest("invalid webhook URL"))?;
    if !matches!(url.scheme(), "https")
        || url.host_str().is_none()
        || url.username() != ""
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(ApiError::BadRequest("invalid webhook URL"));
    }
    let secret_ciphertext = request
        .secret
        .map(Zeroizing::new)
        .filter(|secret| !secret.is_empty())
        .map(|secret| state.control.worker_cipher().encrypt(secret.as_bytes()))
        .transpose()?;
    Ok(NewWebhook {
        name: name.to_owned(),
        url: url.to_string(),
        secret_ciphertext,
        is_enabled: request.is_enabled.unwrap_or(true),
    })
}
