use axum::{Json, extract::State, http::HeaderMap};
use ignitify_db::{DeploymentActor, ProjectActor, ServiceActor};
use serde::Serialize;

use crate::{
    error::ApiError, extract::require_actor, handlers::deployments::DeploymentResponse,
    state::AppState,
};

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

pub(crate) async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DashboardResponse>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let project_actor = ProjectActor {
        id: &actor.id,
        is_admin: actor.has_admin_access(),
    };
    let service_actor = ServiceActor {
        id: &actor.id,
        is_admin: actor.has_admin_access(),
    };
    let deployment_actor = DeploymentActor {
        id: &actor.id,
        is_admin: actor.has_admin_access(),
    };

    let projects = state.database.projects().list(project_actor).await?;
    let mut response = DashboardResponse {
        projects: projects
            .iter()
            .map(|project| DashboardProjectResponse {
                id: project.id.to_string(),
                name: project.name.clone(),
            })
            .collect(),
        services: Vec::new(),
        deployments: Vec::new(),
    };

    for project in projects {
        let project_id = project.id.to_string();
        let services = state
            .database
            .services()
            .list(service_actor, &project_id)
            .await?
            .ok_or(ApiError::NotFound)?;
        response.services.extend(
            services
                .into_iter()
                .map(|service| DashboardServiceResponse {
                    id: service.id.to_string(),
                    project_id: service.project_id.to_string(),
                    name: service.name,
                    kind: service.kind.as_str().to_owned(),
                    desired_generation: service.desired_generation,
                    desired_state: service.desired_state,
                }),
        );

        let deployments = state
            .control
            .list_for_project(deployment_actor, &project_id, None, None)
            .await?
            .ok_or(ApiError::NotFound)?;
        response
            .deployments
            .extend(deployments.into_iter().map(DeploymentResponse::from));
    }

    Ok(Json(response))
}
