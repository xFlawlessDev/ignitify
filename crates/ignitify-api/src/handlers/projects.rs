use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use ignitify_auth::AuthenticatedUser;
use ignitify_db::{ProjectActor, ProjectRemoveOutcome, ProjectUpdateOutcome};
use ignitify_domain::{EnvironmentSummary, ProjectInput, ProjectSummary};
use serde::{Deserialize, Serialize};

use crate::{
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

#[derive(Serialize)]
pub(crate) struct ProjectResponse {
    id: String,
    name: String,
    owner_id: String,
    role: &'static str,
    created_at: String,
    updated_at: String,
    default_environment: EnvironmentResponse,
}

#[derive(Serialize)]
struct EnvironmentResponse {
    id: String,
    name: String,
    is_default: bool,
}

impl From<ProjectSummary> for ProjectResponse {
    fn from(project: ProjectSummary) -> Self {
        Self {
            id: project.id.to_string(),
            name: project.name,
            owner_id: project.owner_id.to_string(),
            role: project.role.as_str(),
            created_at: project.created_at,
            updated_at: project.updated_at,
            default_environment: project.default_environment.into(),
        }
    }
}

impl From<EnvironmentSummary> for EnvironmentResponse {
    fn from(environment: EnvironmentSummary) -> Self {
        Self {
            id: environment.id.to_string(),
            name: environment.name,
            is_default: environment.is_default,
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct ProjectRequest {
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoveProjectRequest {
    confirm_name: String,
}

pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProjectResponse>>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let projects = state
        .database
        .projects()
        .list(project_actor(&actor))
        .await?
        .into_iter()
        .map(ProjectResponse::from)
        .collect();
    Ok(Json(projects))
}

pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ProjectRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let project = state
        .database
        .projects()
        .create(&actor.id, ProjectInput::new(request.name)?)
        .await?;
    Ok((StatusCode::CREATED, Json(project.into())))
}

pub(crate) async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
) -> Result<Json<ProjectResponse>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let project = state
        .database
        .projects()
        .get(project_actor(&actor), &project_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(project.into()))
}

pub(crate) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(request): Json<ProjectRequest>,
) -> Result<Json<ProjectResponse>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let outcome = state
        .database
        .projects()
        .rename(
            project_actor(&actor),
            &project_id,
            ProjectInput::new(request.name)?,
        )
        .await?;
    match outcome {
        ProjectUpdateOutcome::Updated(project) => Ok(Json(project.into())),
        ProjectUpdateOutcome::Missing => Err(ApiError::NotFound),
        ProjectUpdateOutcome::Forbidden => Err(ApiError::Forbidden),
    }
}

pub(crate) async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(request): Json<RemoveProjectRequest>,
) -> Result<StatusCode, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    match state
        .database
        .projects()
        .remove(project_actor(&actor), &project_id, &request.confirm_name)
        .await?
    {
        ProjectRemoveOutcome::Removed => Ok(StatusCode::NO_CONTENT),
        ProjectRemoveOutcome::Missing => Err(ApiError::NotFound),
        ProjectRemoveOutcome::Forbidden => Err(ApiError::Forbidden),
    }
}

fn project_actor(actor: &AuthenticatedUser) -> ProjectActor<'_> {
    ProjectActor {
        id: &actor.id,
        is_admin: actor.has_admin_access(),
    }
}
