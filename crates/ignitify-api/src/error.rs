use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use ignitify_auth::AuthError;
use ignitify_control_plane::Error as ControlError;
use ignitify_db::DatabaseError;
use ignitify_runtime_docker::Error as DockerRuntimeError;
use ignitify_terminal::TerminalError;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum ApiError {
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error(transparent)]
    Domain(#[from] ignitify_domain::InputError),
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    Control(#[from] ControlError),
    #[error(transparent)]
    Terminal(#[from] TerminalError),
    #[error(transparent)]
    DockerRuntime(#[from] DockerRuntimeError),
    #[error("not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
    #[error("invalid request")]
    BadRequest(&'static str),
    #[error("compose policy rejected input: {0}")]
    ComposePolicy(&'static str),
    #[error("active deployment exists")]
    ActiveDeploymentConflict,
    #[error("deployment capability unavailable")]
    CapabilityUnavailable,
    #[error("Docker runtime unavailable")]
    DockerCapabilityUnavailable,
    #[error("provider secret storage unavailable")]
    ProviderCapabilityUnavailable,
    #[error("provider remote request failed")]
    ProviderRemote(#[from] reqwest::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::Auth(AuthError::AlreadyBootstrapped) => (
                StatusCode::CONFLICT,
                "bootstrap has already been completed".to_owned(),
            ),
            Self::Auth(
                AuthError::InvalidCredentials | AuthError::InactiveUser | AuthError::InvalidToken,
            ) => (StatusCode::UNAUTHORIZED, "unauthorized".to_owned()),
            Self::Auth(AuthError::InvalidRequest) => {
                (StatusCode::BAD_REQUEST, "invalid request".to_owned())
            }
            Self::Domain(error) => (StatusCode::BAD_REQUEST, error.to_string()),
            Self::Database(DatabaseError::ProjectNameConflict) => (
                StatusCode::CONFLICT,
                "project name already exists".to_owned(),
            ),
            Self::Database(DatabaseError::ProviderNameConflict) => (
                StatusCode::CONFLICT,
                "provider name already exists".to_owned(),
            ),
            Self::Database(DatabaseError::ServiceNameConflict)
            | Self::Control(ignitify_control_plane::Error::Database(
                DatabaseError::ServiceNameConflict,
            )) => (
                StatusCode::CONFLICT,
                "service name already exists".to_owned(),
            ),
            Self::Database(DatabaseError::DomainNameConflict) => (
                StatusCode::CONFLICT,
                "domain hostname already exists".to_owned(),
            ),
            Self::Database(DatabaseError::DomainConfirmationMismatch) => (
                StatusCode::BAD_REQUEST,
                "domain confirmation does not match hostname".to_owned(),
            ),
            Self::Control(ControlError::InvalidIdempotencyKey) => (
                StatusCode::BAD_REQUEST,
                "invalid idempotency key".to_owned(),
            ),
            Self::Control(ControlError::Policy(message)) => (
                StatusCode::BAD_REQUEST,
                format!("compose policy rejected input: {message}"),
            ),
            Self::ComposePolicy(message) => (
                StatusCode::BAD_REQUEST,
                format!("compose policy rejected input: {message}"),
            ),
            Self::Control(ControlError::Domain(error)) => {
                (StatusCode::BAD_REQUEST, error.to_string())
            }
            Self::NotFound => (StatusCode::NOT_FOUND, "not found".to_owned()),
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden".to_owned()),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message.to_owned()),
            Self::ActiveDeploymentConflict => (
                StatusCode::CONFLICT,
                "an active deployment already exists for this service".to_owned(),
            ),
            Self::CapabilityUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "deployment capability is unavailable".to_owned(),
            ),
            Self::DockerCapabilityUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Docker runtime is unavailable".to_owned(),
            ),
            Self::ProviderCapabilityUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "provider capability is unavailable".to_owned(),
            ),
            Self::ProviderRemote(_) => (
                StatusCode::BAD_GATEWAY,
                "provider integration request failed".to_owned(),
            ),
            Self::Terminal(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "host terminal is unavailable".to_owned(),
            ),
            Self::DockerRuntime(DockerRuntimeError::ContainerNotFound) => {
                (StatusCode::NOT_FOUND, "not found".to_owned())
            }
            Self::DockerRuntime(
                DockerRuntimeError::InvalidContainerReference
                | DockerRuntimeError::InvalidUploadPath,
            ) => (
                StatusCode::BAD_REQUEST,
                "invalid container request".to_owned(),
            ),
            Self::DockerRuntime(
                DockerRuntimeError::Connection | DockerRuntimeError::TerminalUnavailable,
            ) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Docker runtime is unavailable".to_owned(),
            ),
            Self::DockerRuntime(DockerRuntimeError::ContainerNotRunning) => {
                (StatusCode::CONFLICT, "container is not running".to_owned())
            }
            Self::Auth(_) | Self::Control(_) | Self::Database(_) | Self::DockerRuntime(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error".to_owned(),
            ),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::to_bytes, response::IntoResponse};

    use super::{ApiError, ControlError};

    #[tokio::test]
    async fn compose_policy_error_maps_to_bad_request() {
        let response = ApiError::Control(ControlError::Policy("invalid YAML")).into_response();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();

        assert_eq!(
            String::from_utf8(body.to_vec()).unwrap(),
            r#"{"error":"compose policy rejected input: invalid YAML"}"#
        );
    }
}
