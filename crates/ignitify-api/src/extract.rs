use axum::{
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
};
use ignitify_auth::{AuthError, AuthSession, AuthenticatedUser};

use crate::{error::ApiError, state::AppState};

pub(crate) async fn require_actor(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthenticatedUser, ApiError> {
    let token = bearer_token(headers).ok_or(AuthError::InvalidToken)?;
    Ok(state.auth.authenticate_bearer(token).await?)
}

pub(crate) fn require_same_origin_request(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(), ApiError> {
    if headers.get("X-Ignitify-Request") != Some(&HeaderValue::from_static("1")) {
        return Err(AuthError::InvalidRequest.into());
    }
    let Some(origin) = headers.get(header::ORIGIN) else {
        return Ok(());
    };
    let origin = origin.to_str().map_err(|_| AuthError::InvalidRequest)?;
    if state
        .trusted_origins
        .iter()
        .any(|trusted| trusted == origin)
    {
        Ok(())
    } else {
        Err(AuthError::InvalidRequest.into())
    }
}

pub(crate) fn session_response(
    secure_cookies: bool,
    mut session: AuthSession,
) -> Result<Response, ApiError> {
    let refresh_token = session
        .refresh_token
        .take()
        .ok_or(AuthError::InvalidToken)?;
    session.refresh_token_expires_at = None;
    Ok(with_refresh_cookie(
        axum::Json(session).into_response(),
        &refresh_cookie_header(&refresh_token, secure_cookies),
    ))
}

pub(crate) fn refresh_cookie(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|entry| entry.strip_prefix("ignitify_refresh="))
}

pub(crate) fn with_refresh_cookie(mut response: Response, cookie: &str) -> Response {
    if let Ok(value) = HeaderValue::from_str(cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    response
}

pub(crate) fn clear_refresh_cookie(secure: bool) -> String {
    cookie("", "Max-Age=0", secure)
}

fn refresh_cookie_header(token: &str, secure: bool) -> String {
    cookie(token, "Max-Age=604800", secure)
}

fn cookie(token: &str, max_age: &str, secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!(
        "ignitify_refresh={token}; Path=/api/v1/auth; HttpOnly; SameSite=Lax; {max_age}{secure}"
    )
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
}
