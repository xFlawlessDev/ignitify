use axum::{
    Json,
    extract::{Path, State},
};
use ignitify_db::{DomainActor, DomainMutationOutcome, DomainRecord};
use ignitify_domain::DomainName;
use serde::{Deserialize, Serialize};

use crate::{
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateDomainRequest {
    hostname: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoveDomainRequest {
    confirm_hostname: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct DomainResponse {
    id: String,
    service_id: String,
    hostname: String,
    status: String,
    last_error: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<DomainRecord> for DomainResponse {
    fn from(domain: DomainRecord) -> Self {
        Self {
            id: domain.id.to_string(),
            service_id: domain.service_id.to_string(),
            hostname: domain.hostname.to_string(),
            status: domain.status.as_str().to_owned(),
            last_error: domain.last_error,
            created_at: domain.created_at,
            updated_at: domain.updated_at,
        }
    }
}

pub(crate) async fn list(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(service_id): Path<String>,
) -> Result<Json<Vec<DomainResponse>>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let domains = state
        .database
        .domains()
        .list(domain_actor(&actor), &service_id)
        .await?
        .ok_or(ApiError::NotFound)?
        .into_iter()
        .map(DomainResponse::from)
        .collect();
    Ok(Json(domains))
}

pub(crate) async fn create(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(service_id): Path<String>,
    Json(input): Json<CreateDomainRequest>,
) -> Result<(axum::http::StatusCode, Json<DomainResponse>), ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let hostname = DomainName::new(input.hostname)?;
    if !state.domain_policy.allows(&hostname) {
        return Err(ApiError::BadRequest(
            "hostname is not allowed by this server's domain policy",
        ));
    }
    let service = state
        .services()?
        .get(
            ignitify_db::ServiceActor {
                id: &actor.id,
                is_admin: actor.has_admin_access(),
            },
            &service_id,
        )
        .await?
        .ok_or(ApiError::NotFound)?;
    let has_port = service.spec.internal_port().is_some();
    if !has_port {
        return Err(ApiError::BadRequest(
            "service needs internal port before adding domain",
        ));
    }
    let outcome = state
        .database
        .domains()
        .create(domain_actor(&actor), &service_id, hostname)
        .await?;
    let record = match outcome {
        DomainMutationOutcome::Created(record) => record,
        DomainMutationOutcome::Missing => return Err(ApiError::NotFound),
        DomainMutationOutcome::Forbidden => return Err(ApiError::Forbidden),
        DomainMutationOutcome::Removed(_) => {
            return Err(ApiError::BadRequest("invalid domain operation"));
        }
    };
    let _ = state.control()?.wake_worker();
    Ok((axum::http::StatusCode::ACCEPTED, Json(record.into())))
}

pub(crate) async fn remove(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(domain_id): Path<String>,
    Json(input): Json<RemoveDomainRequest>,
) -> Result<(axum::http::StatusCode, Json<DomainResponse>), ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let outcome = state
        .database
        .domains()
        .remove(domain_actor(&actor), &domain_id, &input.confirm_hostname)
        .await?;
    let record = match outcome {
        DomainMutationOutcome::Removed(record) => record,
        DomainMutationOutcome::Missing => return Err(ApiError::NotFound),
        DomainMutationOutcome::Forbidden => return Err(ApiError::Forbidden),
        DomainMutationOutcome::Created(_) => {
            return Err(ApiError::BadRequest("invalid domain operation"));
        }
    };
    let _ = state.control()?.wake_worker();
    Ok((axum::http::StatusCode::ACCEPTED, Json(record.into())))
}

fn domain_actor(actor: &ignitify_auth::AuthenticatedUser) -> DomainActor<'_> {
    DomainActor {
        id: &actor.id,
        is_admin: actor.has_admin_access(),
    }
}
