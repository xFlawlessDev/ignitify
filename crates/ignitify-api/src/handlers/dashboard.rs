use axum::{Json, extract::State, http::HeaderMap};
use ignitify_db::{
    DashboardActor, DashboardDeploymentRecord, DashboardProjectRecord, DashboardServiceRecord,
};
use serde::Serialize;

use crate::{error::ApiError, extract::require_actor, state::AppState};

#[derive(Debug, Serialize)]
pub(crate) struct DashboardResponse {
    projects: Vec<DashboardProjectResponse>,
    services: Vec<DashboardServiceResponse>,
    deployments: Vec<DeploymentResponse>,
}

#[derive(Debug, Serialize)]
struct DashboardProjectResponse {
    id: String,
    name: String,
}

#[derive(Debug, Serialize)]
struct DashboardServiceResponse {
    id: String,
    project_id: String,
    name: String,
    kind: String,
    desired_generation: i64,
    desired_state: String,
}

#[derive(Debug, Serialize)]
struct DeploymentResponse {
    id: String,
    service_id: String,
    generation: i64,
    status: String,
    failure_reason: Option<String>,
    created_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
}

impl From<DashboardProjectRecord> for DashboardProjectResponse {
    fn from(project: DashboardProjectRecord) -> Self {
        Self {
            id: project.id,
            name: project.name,
        }
    }
}

impl From<DashboardServiceRecord> for DashboardServiceResponse {
    fn from(service: DashboardServiceRecord) -> Self {
        Self {
            id: service.id,
            project_id: service.project_id,
            name: service.name,
            kind: service.kind,
            desired_generation: service.desired_generation,
            desired_state: service.desired_state,
        }
    }
}

impl From<DashboardDeploymentRecord> for DeploymentResponse {
    fn from(deployment: DashboardDeploymentRecord) -> Self {
        Self {
            id: deployment.id,
            service_id: deployment.service_id,
            generation: deployment.generation,
            status: deployment.status,
            failure_reason: deployment.failure_reason,
            created_at: deployment.created_at,
            started_at: deployment.started_at,
            finished_at: deployment.finished_at,
        }
    }
}

pub(crate) async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DashboardResponse>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let summary = state
        .database
        .dashboard()
        .summary(DashboardActor {
            id: &actor.id,
            is_admin: actor.has_platform_operator_access(),
        })
        .await?;
    Ok(Json(DashboardResponse {
        projects: summary
            .projects
            .into_iter()
            .map(DashboardProjectResponse::from)
            .collect(),
        services: summary
            .services
            .into_iter()
            .map(DashboardServiceResponse::from)
            .collect(),
        deployments: summary
            .deployments
            .into_iter()
            .map(DeploymentResponse::from)
            .collect(),
    }))
}
