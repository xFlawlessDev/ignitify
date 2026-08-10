use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use ignitify_control_plane::{
    ServiceMutationOutcomeModel, ServiceReadModel, ServiceVariableReadModel,
};
use ignitify_db::ServiceActor;
use ignitify_domain::{
    ApplicationBuilder, ServiceInput, ServiceSourceConfig, ServiceSpec, ServiceVariableInput,
};
use ignitify_runtime_compose::validate_submission_yaml;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

const APPLICATION_RUNTIME_PLACEHOLDER: &str = "ignitify-source-placeholder@sha256:0000000000000000000000000000000000000000000000000000000000000000";

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
    source_config: Option<ServiceSourceConfig>,
    #[serde(default)]
    deployment_destination_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoveServiceRequest {
    confirm_name: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    source_config: Option<ServiceSourceConfig>,
    deployment_destination_id: Option<String>,
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
            source_config: service.source_config,
            deployment_destination_id: service.deployment_destination_id,
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
        .services()?
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
    let input = input(request)?;
    ensure_source_provider(&state, input.configuration.source_config.as_ref()).await?;
    match state
        .services()?
        .create(service_actor(&actor), &project_id, input)
        .await?
    {
        ServiceMutationOutcomeModel::Created(service) => {
            Ok((StatusCode::CREATED, Json(service.into())))
        }
        ServiceMutationOutcomeModel::Missing => Err(ApiError::NotFound),
        ServiceMutationOutcomeModel::Forbidden => Err(ApiError::Forbidden),
        ServiceMutationOutcomeModel::Updated(_) => unreachable!("service create cannot update"),
        ServiceMutationOutcomeModel::Removed => unreachable!("service create cannot remove"),
    }
}

pub(crate) async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(service_id): Path<String>,
) -> Result<Json<ServiceResponse>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let service = state
        .services()?
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
    let input = input(request)?;
    ensure_source_provider(&state, input.configuration.source_config.as_ref()).await?;
    match state
        .services()?
        .update(service_actor(&actor), &service_id, input)
        .await?
    {
        ServiceMutationOutcomeModel::Updated(service) => Ok(Json(service.into())),
        ServiceMutationOutcomeModel::Missing => Err(ApiError::NotFound),
        ServiceMutationOutcomeModel::Forbidden => Err(ApiError::Forbidden),
        ServiceMutationOutcomeModel::Created(_) => unreachable!("service update cannot create"),
        ServiceMutationOutcomeModel::Removed => unreachable!("service update cannot remove"),
    }
}

pub(crate) async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(service_id): Path<String>,
    Json(input): Json<RemoveServiceRequest>,
) -> Result<StatusCode, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    match state
        .services()?
        .remove(service_actor(&actor), &service_id, &input.confirm_name)
        .await?
    {
        ServiceMutationOutcomeModel::Removed => {
            let _ = state.control()?.wake_worker();
            Ok(StatusCode::NO_CONTENT)
        }
        ServiceMutationOutcomeModel::Missing => Err(ApiError::NotFound),
        ServiceMutationOutcomeModel::Forbidden => Err(ApiError::Forbidden),
        ServiceMutationOutcomeModel::Created(_) => unreachable!("service remove cannot create"),
        ServiceMutationOutcomeModel::Updated(_) => unreachable!("service remove cannot update"),
    }
}

fn input(request: ServiceRequest) -> Result<ServiceInput, ApiError> {
    let source_config = request.source_config;
    let application_source = source_config
        .as_ref()
        .is_some_and(|source| source.source == "application");
    let git_compose_source = source_config
        .as_ref()
        .is_some_and(|source| source.source == "compose" && source.provider_id.is_some());
    let compose_yaml = request
        .compose_yaml
        .filter(|yaml| !yaml.trim().is_empty())
        .unwrap_or_else(|| {
            let exposed_service = request.exposed_service.as_deref().unwrap_or("web");
            if git_compose_source {
                format!(
                    "services:\n  {exposed_service}:\n    image: ignitify-source-placeholder@sha256:{}\n",
                    "0".repeat(64)
                )
            } else {
                String::new()
            }
        });
    let variables = request
        .variables
        .into_iter()
        .map(|variable| ServiceVariableInput {
            key: variable.key,
            value: variable.value,
            is_secret: variable.is_secret,
        })
        .collect();
    let mut input = match request.kind.as_deref().unwrap_or("image") {
        "image" => ServiceInput::image(
            request.name,
            if application_source {
                APPLICATION_RUNTIME_PLACEHOLDER.to_owned()
            } else {
                request.image_reference.unwrap_or_default()
            },
            request.internal_port,
            request.healthcheck,
            variables,
        ),
        "compose" => ServiceInput::compose(
            request.name,
            compose_yaml,
            request.exposed_service.unwrap_or_default(),
            request.internal_port,
            variables,
        ),
        _ => Err(ignitify_domain::InputError::InvalidServiceKind),
    }?;
    if let Some(source_config) = source_config {
        source_config.validate()?;
        if source_config.source == "compose"
            && !matches!(&input.configuration.spec, ServiceSpec::Compose { .. })
        {
            return Err(ignitify_domain::InputError::InvalidServiceSourceConfig.into());
        }
        if source_config.source == "application"
            && (!matches!(&input.configuration.spec, ServiceSpec::Image { .. })
                || source_config.builder == Some(ApplicationBuilder::Spa)
                || (source_config.builder == Some(ApplicationBuilder::Static)
                    && input.configuration.spec.internal_port() != Some(80)))
        {
            return Err(ignitify_domain::InputError::InvalidServiceSourceConfig.into());
        }
        input.configuration.source_config = Some(source_config);
    }
    input.configuration.deployment_destination_id = request.deployment_destination_id;
    if !git_compose_source && let ServiceSpec::Compose { yaml, .. } = &input.configuration.spec {
        validate_submission_yaml(yaml).map_err(ApiError::ComposePolicy)?;
    }
    Ok(input)
}

fn service_actor(actor: &ignitify_auth::AuthenticatedUser) -> ServiceActor<'_> {
    ServiceActor {
        id: &actor.id,
        is_admin: actor.has_platform_operator_access(),
    }
}

async fn ensure_source_provider(
    state: &AppState,
    source_config: Option<&ServiceSourceConfig>,
) -> Result<(), ApiError> {
    let Some(provider_id) = source_config.and_then(|config| config.provider_id.as_deref()) else {
        return Ok(());
    };
    if state.database.providers().get(provider_id).await?.is_none() {
        return Err(ApiError::BadRequest("selected provider was not found"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        APPLICATION_RUNTIME_PLACEHOLDER, ServiceRequest, ServiceVariableReadModel,
        ServiceVariableResponse, input,
    };
    use crate::error::ApiError;
    use ignitify_domain::{ApplicationBuilder, ServiceSourceConfig, ServiceSpec};

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

    #[test]
    fn compose_policy_rejects_host_escape_before_persistence() {
        let result = input(ServiceRequest {
            name: "web".to_owned(),
            kind: Some("compose".to_owned()),
            image_reference: None,
            compose_yaml: Some(
                "services:\n  web:\n    image: nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n    ports: [\"8080:80\"]\n"
                    .to_owned(),
            ),
            exposed_service: Some("web".to_owned()),
            internal_port: Some(80),
            healthcheck: None,
            variables: vec![],
            source_config: None,
            deployment_destination_id: None,
        });

        assert!(matches!(result, Err(ApiError::ComposePolicy(_))));
    }

    #[test]
    fn compose_policy_rejects_mutable_image_tags() {
        let result = input(ServiceRequest {
            name: "wordpress".to_owned(),
            kind: Some("compose".to_owned()),
            image_reference: None,
            compose_yaml: Some("services:\n  wordpress:\n    image: wordpress:latest\n".to_owned()),
            exposed_service: Some("wordpress".to_owned()),
            internal_port: Some(80),
            healthcheck: None,
            variables: vec![],
            source_config: None,
            deployment_destination_id: None,
        });

        assert!(matches!(result, Err(ApiError::ComposePolicy(_))));
    }

    #[test]
    fn compose_policy_accepts_safe_prebuilt_image() {
        let result = input(ServiceRequest {
            name: "web".to_owned(),
            kind: Some("compose".to_owned()),
            image_reference: None,
            compose_yaml: Some(
                "services:\n  web:\n    image: nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n"
                    .to_owned(),
            ),
            exposed_service: Some("web".to_owned()),
            internal_port: Some(80),
            healthcheck: None,
            variables: vec![],
            source_config: None,
            deployment_destination_id: None,
        });

        assert!(result.is_ok());
    }

    #[test]
    fn git_compose_source_uses_a_runtime_placeholder_until_checkout() {
        let result = input(ServiceRequest {
            name: "web".to_owned(),
            kind: Some("compose".to_owned()),
            image_reference: None,
            compose_yaml: None,
            exposed_service: Some("web".to_owned()),
            internal_port: Some(80),
            healthcheck: None,
            variables: vec![],
            source_config: Some(ServiceSourceConfig {
                source: "compose".to_owned(),
                template: None,
                setup_required: None,
                provider_id: Some("provider-1".to_owned()),
                repository: Some("acme/stack".to_owned()),
                branch: Some("main".to_owned()),
                builder: None,
                dockerfile_path: Some("deploy/compose.yaml".to_owned()),
                build_command: None,
                output_directory: None,
            }),
            deployment_destination_id: None,
        })
        .unwrap();

        let ServiceSpec::Compose { yaml, .. } = result.configuration.spec else {
            panic!("expected Compose service specification");
        };
        assert!(yaml.contains("ignitify-source-placeholder@sha256:"));
    }

    #[test]
    fn git_compose_source_requires_a_compose_service() {
        let result = input(ServiceRequest {
            name: "web".to_owned(),
            kind: Some("image".to_owned()),
            image_reference: Some(
                "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_owned(),
            ),
            compose_yaml: None,
            exposed_service: None,
            internal_port: Some(80),
            healthcheck: None,
            variables: vec![],
            source_config: Some(ServiceSourceConfig {
                source: "compose".to_owned(),
                template: None,
                setup_required: None,
                provider_id: Some("provider-1".to_owned()),
                repository: Some("acme/stack".to_owned()),
                branch: Some("main".to_owned()),
                builder: None,
                dockerfile_path: None,
                build_command: None,
                output_directory: None,
            }),
            deployment_destination_id: None,
        });

        assert!(matches!(
            result,
            Err(ApiError::Domain(
                ignitify_domain::InputError::InvalidServiceSourceConfig
            ))
        ));
    }

    #[test]
    fn application_source_configuration_is_part_of_service_input() {
        let result = input(ServiceRequest {
            name: "web".to_owned(),
            kind: Some("image".to_owned()),
            image_reference: None,
            compose_yaml: None,
            exposed_service: None,
            internal_port: Some(80),
            healthcheck: None,
            variables: vec![],
            source_config: Some(ServiceSourceConfig {
                source: "application".to_owned(),
                template: None,
                setup_required: None,
                provider_id: Some("provider-1".to_owned()),
                repository: Some("acme/site".to_owned()),
                branch: Some("main".to_owned()),
                builder: Some(ApplicationBuilder::Static),
                dockerfile_path: None,
                build_command: None,
                output_directory: None,
            }),
            deployment_destination_id: None,
        })
        .unwrap();

        assert_eq!(
            result
                .configuration
                .source_config
                .as_ref()
                .and_then(|config| config.repository.as_deref()),
            Some("acme/site")
        );
        assert!(matches!(
            result.configuration.spec,
            ServiceSpec::Image { image_reference, .. }
                if image_reference == APPLICATION_RUNTIME_PLACEHOLDER
        ));
    }

    #[test]
    fn application_source_rejects_unsupported_builder_and_static_port_mismatch() {
        let request = |builder, internal_port| ServiceRequest {
            name: "web".to_owned(),
            kind: Some("image".to_owned()),
            image_reference: Some(
                "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_owned(),
            ),
            compose_yaml: None,
            exposed_service: None,
            internal_port,
            healthcheck: None,
            variables: vec![],
            source_config: Some(ServiceSourceConfig {
                source: "application".to_owned(),
                template: None,
                setup_required: None,
                provider_id: Some("provider-1".to_owned()),
                repository: Some("acme/site".to_owned()),
                branch: Some("main".to_owned()),
                builder: Some(builder),
                dockerfile_path: None,
                build_command: None,
                output_directory: None,
            }),
            deployment_destination_id: None,
        };

        assert!(matches!(
            input(request(ApplicationBuilder::Spa, Some(80))),
            Err(ApiError::Domain(
                ignitify_domain::InputError::InvalidServiceSourceConfig
            ))
        ));
        assert!(matches!(
            input(request(ApplicationBuilder::Static, Some(3000))),
            Err(ApiError::Domain(
                ignitify_domain::InputError::InvalidServiceSourceConfig
            ))
        ));
    }
}
