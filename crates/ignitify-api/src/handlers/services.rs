use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use ignitify_control_plane::{
    ServiceMutationOutcomeModel, ServiceReadModel, ServiceVariableReadModel,
};
use ignitify_db::ServiceActor;
use ignitify_domain::{ServiceInput, ServiceSpec, ServiceVariableInput};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub(crate) struct ServiceRequest {
    name: String,
    kind: Option<String>,
    image_reference: Option<String>,
    compose_yaml: Option<String>,
    exposed_service: Option<String>,
    internal_port: Option<u32>,
    healthcheck: Option<Vec<String>>,
    variables: Vec<ServiceVariableRequest>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ServiceVariableRequest {
    key: String,
    value: String,
    is_secret: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ServiceResponse {
    id: String,
    project_id: String,
    environment_id: String,
    role: String,
    name: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compose_yaml: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exposed_service: Option<String>,
    internal_port: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    healthcheck: Option<Vec<String>>,
    desired_generation: i64,
    desired_state: String,
    created_at: String,
    updated_at: String,
    variables: Vec<ServiceVariableResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ServiceVariableResponse {
    key: String,
    is_secret: bool,
    is_set: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<Zeroizing<String>>,
}

impl From<ServiceReadModel> for ServiceResponse {
    fn from(service: ServiceReadModel) -> Self {
        let (image_reference, compose_yaml, exposed_service, internal_port, healthcheck) =
            match service.spec {
                ServiceSpec::Image {
                    image_reference,
                    internal_port,
                    healthcheck,
                } => (
                    Some(image_reference),
                    None,
                    None,
                    internal_port,
                    healthcheck,
                ),
                ServiceSpec::Compose {
                    yaml,
                    exposed_service,
                    internal_port,
                } => (None, Some(yaml), Some(exposed_service), internal_port, None),
            };
        Self {
            id: service.id,
            project_id: service.project_id,
            environment_id: service.environment_id,
            role: service.role,
            name: service.name,
            kind: service.kind,
            image_reference,
            compose_yaml,
            exposed_service,
            internal_port,
            healthcheck,
            desired_generation: service.desired_generation,
            desired_state: service.desired_state,
            created_at: service.created_at,
            updated_at: service.updated_at,
            variables: service
                .variables
                .into_iter()
                .map(ServiceVariableResponse::from)
                .collect(),
        }
    }
}

impl From<ServiceVariableReadModel> for ServiceVariableResponse {
    fn from(variable: ServiceVariableReadModel) -> Self {
        Self {
            key: variable.key,
            is_secret: variable.is_secret,
            is_set: variable.is_set,
            value: variable.value,
        }
    }
}

pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
) -> Result<Json<Vec<ServiceResponse>>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let services = state
        .services
        .list(service_actor(&actor), &project_id)
        .await?
        .ok_or(ApiError::NotFound)?
        .into_iter()
        .map(ServiceResponse::from)
        .collect();
    Ok(Json(services))
}

pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(request): Json<ServiceRequest>,
) -> Result<(StatusCode, Json<ServiceResponse>), ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    match state
        .services
        .create(service_actor(&actor), &project_id, input(request)?)
        .await?
    {
        ServiceMutationOutcomeModel::Created(service) => {
            Ok((StatusCode::CREATED, Json(service.into())))
        }
        ServiceMutationOutcomeModel::Missing => Err(ApiError::NotFound),
        ServiceMutationOutcomeModel::Forbidden => Err(ApiError::Forbidden),
        ServiceMutationOutcomeModel::Updated(_) => unreachable!("service create cannot update"),
    }
}

pub(crate) async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(service_id): Path<String>,
) -> Result<Json<ServiceResponse>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let service = state
        .services
        .get(service_actor(&actor), &service_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(service.into()))
}

pub(crate) async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(service_id): Path<String>,
    Json(request): Json<ServiceRequest>,
) -> Result<Json<ServiceResponse>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    match state
        .services
        .update(service_actor(&actor), &service_id, input(request)?)
        .await?
    {
        ServiceMutationOutcomeModel::Updated(service) => Ok(Json(service.into())),
        ServiceMutationOutcomeModel::Missing => Err(ApiError::NotFound),
        ServiceMutationOutcomeModel::Forbidden => Err(ApiError::Forbidden),
        ServiceMutationOutcomeModel::Created(_) => unreachable!("service update cannot create"),
    }
}

fn input(request: ServiceRequest) -> Result<ServiceInput, ignitify_domain::InputError> {
    let variables = request
        .variables
        .into_iter()
        .map(|variable| ServiceVariableInput {
            key: variable.key,
            value: variable.value,
            is_secret: variable.is_secret,
        })
        .collect();
    match request.kind.as_deref().unwrap_or("image") {
        "image" => ServiceInput::image(
            request.name,
            request.image_reference.unwrap_or_default(),
            request.internal_port,
            request.healthcheck,
            variables,
        ),
        "compose" => ServiceInput::compose(
            request.name,
            request.compose_yaml.unwrap_or_default(),
            request.exposed_service.unwrap_or_default(),
            request.internal_port,
            variables,
        ),
        _ => Err(ignitify_domain::InputError::InvalidServiceKind),
    }
}

fn service_actor(actor: &ignitify_auth::AuthenticatedUser) -> ServiceActor<'_> {
    ServiceActor {
        id: &actor.id,
        is_admin: actor.has_admin_access(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ServiceVariableReadModel, ServiceVariableResponse};

    #[test]
    fn secret_response_omits_value() {
        let variable = ServiceVariableResponse::from(ServiceVariableReadModel {
            key: "TOKEN".to_owned(),
            is_secret: true,
            is_set: true,
            value: None,
        });
        let value = serde_json::to_value(variable).unwrap();

        assert!(value.get("value").is_none());
    }
}
