use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use ignitify_control_plane::DeploymentSubmission;
use ignitify_db::{DeploymentActor, DeploymentRecord};
use serde::{Deserialize, Serialize};

use crate::{
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub(crate) struct DeploymentListQuery {
    before: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DeploymentResponse {
    pub(crate) id: String,
    pub(crate) service_id: String,
    pub(crate) generation: i64,
    pub(crate) status: String,
    pub(crate) failure_reason: Option<String>,
    pub(crate) created_at: String,
    pub(crate) started_at: Option<String>,
    pub(crate) finished_at: Option<String>,
}

impl From<DeploymentRecord> for DeploymentResponse {
    fn from(deployment: DeploymentRecord) -> Self {
        Self {
            id: deployment.id.to_string(),
            service_id: deployment.service_id.to_string(),
            generation: deployment.generation,
            status: deployment.state.as_str().to_owned(),
            failure_reason: deployment.failure_reason,
            created_at: deployment.created_at,
            started_at: deployment.started_at,
            finished_at: deployment.finished_at,
        }
    }
}

pub(crate) async fn deploy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(service_id): Path<String>,
) -> Result<(StatusCode, Json<DeploymentResponse>), ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiError::BadRequest("Idempotency-Key is required"))?;
    let deployment = submission_record(
        state
            .control()?
            .submit_deploy(deployment_actor(&actor), &service_id, key)
            .await?,
    )?;
    Ok((StatusCode::ACCEPTED, Json(deployment.into())))
}

pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(service_id): Path<String>,
    Query(query): Query<DeploymentListQuery>,
) -> Result<Json<Vec<DeploymentResponse>>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let deployments = state
        .control()?
        .list(
            deployment_actor(&actor),
            &service_id,
            query.before.as_deref(),
            query.limit,
        )
        .await?
        .ok_or(ApiError::NotFound)?
        .into_iter()
        .map(DeploymentResponse::from)
        .collect();
    Ok(Json(deployments))
}

pub(crate) async fn list_for_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Query(query): Query<DeploymentListQuery>,
) -> Result<Json<Vec<DeploymentResponse>>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let deployments = state
        .control()?
        .list_for_project(
            deployment_actor(&actor),
            &project_id,
            query.before.as_deref(),
            query.limit,
        )
        .await?
        .ok_or(ApiError::NotFound)?
        .into_iter()
        .map(DeploymentResponse::from)
        .collect();
    Ok(Json(deployments))
}

pub(crate) async fn stop(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(service_id): Path<String>,
) -> Result<(StatusCode, Json<DeploymentResponse>), ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let deployment = submission_record(
        state
            .control()?
            .submit_stop(deployment_actor(&actor), &service_id)
            .await?,
    )?;
    Ok((StatusCode::ACCEPTED, Json(deployment.into())))
}

pub(crate) async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(deployment_id): Path<String>,
) -> Result<Json<DeploymentResponse>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let deployment = state
        .control()?
        .get(deployment_actor(&actor), &deployment_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(deployment.into()))
}

pub(crate) async fn rollback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(deployment_id): Path<String>,
) -> Result<(StatusCode, Json<DeploymentResponse>), ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiError::BadRequest("Idempotency-Key is required"))?;
    let outcome = state
        .control()?
        .submit_rollback(deployment_actor(&actor), &deployment_id, key)
        .await?;
    let deployment = submission_record(outcome)?;
    Ok((StatusCode::ACCEPTED, Json(deployment.into())))
}

fn submission_record(outcome: DeploymentSubmission) -> Result<DeploymentRecord, ApiError> {
    match outcome {
        DeploymentSubmission::Accepted(record) | DeploymentSubmission::Existing(record) => {
            Ok(record)
        }
        DeploymentSubmission::Missing => Err(ApiError::NotFound),
        DeploymentSubmission::Forbidden => Err(ApiError::Forbidden),
        DeploymentSubmission::ActiveConflict => Err(ApiError::ActiveDeploymentConflict),
    }
}

fn deployment_actor(actor: &ignitify_auth::AuthenticatedUser) -> DeploymentActor<'_> {
    DeploymentActor {
        id: &actor.id,
        is_admin: actor.has_admin_access(),
    }
}
