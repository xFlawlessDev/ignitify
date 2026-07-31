use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use ignitify_auth::AuthError;
use ignitify_db::DatabaseError;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum ApiError {
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error(transparent)]
    Domain(#[from] ignitify_domain::InputError),
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error("not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
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
            Self::NotFound => (StatusCode::NOT_FOUND, "not found".to_owned()),
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden".to_owned()),
            Self::Auth(_) | Self::Database(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error".to_owned(),
            ),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
