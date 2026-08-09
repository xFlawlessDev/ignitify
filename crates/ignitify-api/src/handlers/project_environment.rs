use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use ignitify_auth::AuthenticatedUser;
use ignitify_control_plane::{
    ProjectEnvironmentMutationModel, ProjectEnvironmentReadModel, ProjectEnvironmentVariableInput,
    ProjectEnvironmentVariableReadModel,
};
use ignitify_db::ProjectActor;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

#[derive(Debug, Serialize)]
pub(crate) struct ProjectEnvironmentResponse {
    role: String,
    variables: Vec<ProjectEnvironmentVariableResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ProjectEnvironmentVariableResponse {
    key: String,
    is_secret: bool,
    is_set: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<Zeroizing<String>>,
}

impl From<ProjectEnvironmentReadModel> for ProjectEnvironmentResponse {
    fn from(environment: ProjectEnvironmentReadModel) -> Self {
        Self {
            role: environment.role,
            variables: environment
                .variables
                .into_iter()
                .map(ProjectEnvironmentVariableResponse::from)
                .collect(),
        }
    }
}

impl From<ProjectEnvironmentVariableReadModel> for ProjectEnvironmentVariableResponse {
    fn from(variable: ProjectEnvironmentVariableReadModel) -> Self {
        Self {
            key: variable.key,
            is_secret: variable.is_secret,
            is_set: variable.is_set,
            value: variable.value,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProjectEnvironmentRequest {
    variables: Vec<ProjectEnvironmentVariableRequest>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProjectEnvironmentVariableRequest {
    key: String,
    value: Option<String>,
    is_secret: bool,
}

pub(crate) async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
) -> Result<Json<ProjectEnvironmentResponse>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let environment = state
        .services()?
        .project_environment(project_actor(&actor), &project_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(environment.into()))
}

pub(crate) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(request): Json<ProjectEnvironmentRequest>,
) -> Result<Json<ProjectEnvironmentResponse>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    match state
        .services()?
        .update_project_environment(
            project_actor(&actor),
            &project_id,
            request
                .variables
                .into_iter()
                .map(ProjectEnvironmentVariableInput::from)
                .collect(),
        )
        .await?
    {
        ProjectEnvironmentMutationModel::Updated(environment) => Ok(Json(environment.into())),
        ProjectEnvironmentMutationModel::Missing => Err(ApiError::NotFound),
        ProjectEnvironmentMutationModel::Forbidden => Err(ApiError::Forbidden),
    }
}

impl From<ProjectEnvironmentVariableRequest> for ProjectEnvironmentVariableInput {
    fn from(variable: ProjectEnvironmentVariableRequest) -> Self {
        Self {
            key: variable.key,
            value: variable.value,
            is_secret: variable.is_secret,
        }
    }
}

fn project_actor(actor: &AuthenticatedUser) -> ProjectActor<'_> {
    ProjectActor {
        id: &actor.id,
        is_admin: actor.has_platform_operator_access(),
    }
}
