use axum::{
    Json,
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use ignitify_auth::{AuthError, BootstrapRequest, LoginRequest};
use serde::Serialize;

use crate::{
    error::ApiError,
    extract::{
        clear_refresh_cookie, refresh_cookie, require_actor, require_same_origin_request,
        session_response, with_refresh_cookie,
    },
    state::AppState,
};

#[derive(Serialize)]
pub(crate) struct BootstrapStatus {
    required: bool,
}

#[derive(Serialize)]
struct MessageResponse {
    message: &'static str,
}

pub(crate) async fn bootstrap_status(
    State(state): State<AppState>,
) -> Result<Json<BootstrapStatus>, ApiError> {
    let required = state.auth.bootstrap_required().await?;
    Ok(Json(BootstrapStatus { required }))
}

pub(crate) async fn bootstrap(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BootstrapRequest>,
) -> Result<Response, ApiError> {
    require_same_origin_request(&state, &headers)?;
    session_response(
        state.secure_cookies,
        state
            .auth
            .bootstrap_admin(&request.username, &request.password)
            .await?,
    )
}

pub(crate) async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Response, ApiError> {
    require_same_origin_request(&state, &headers)?;
    session_response(
        state.secure_cookies,
        state
            .auth
            .login(&request.username, &request.password)
            .await?,
    )
}

pub(crate) async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    require_same_origin_request(&state, &headers)?;
    let token = refresh_cookie(&headers).ok_or(AuthError::InvalidToken)?;
    session_response(
        state.secure_cookies,
        state.auth.refresh_session(token).await?,
    )
}

pub(crate) async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    require_same_origin_request(&state, &headers)?;
    if let Some(token) = refresh_cookie(&headers) {
        state.auth.revoke_refresh_token(token).await?;
    }
    Ok(with_refresh_cookie(
        Json(MessageResponse {
            message: "logged out",
        })
        .into_response(),
        &clear_refresh_cookie(state.secure_cookies),
    ))
}

pub(crate) async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ignitify_auth::AuthenticatedUser>, ApiError> {
    Ok(Json(require_actor(&state, &headers).await?))
}
