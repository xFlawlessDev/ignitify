use axum::{
    Json,
    extract::{ConnectInfo, Extension, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use ignitify_auth::{AuthError, BootstrapRequest, LoginRequest, StepUpRequest, StepUpSession};
use ignitify_db::AuditOutcome;
use serde::Serialize;
use std::net::SocketAddr;

use crate::{
    audit,
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
    enabled: bool,
}

#[derive(Serialize)]
struct MessageResponse {
    message: &'static str,
}

pub(crate) async fn bootstrap_status(
    State(state): State<AppState>,
) -> Result<Json<BootstrapStatus>, ApiError> {
    let required = state.auth.bootstrap_required().await?;
    Ok(Json(BootstrapStatus {
        required,
        enabled: state.auth.bootstrap_enabled(),
    }))
}

pub(crate) async fn bootstrap(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    Json(request): Json<BootstrapRequest>,
) -> Result<Response, ApiError> {
    require_same_origin_request(&state, &headers)?;
    let source = rate_limit_source(&state, &headers, peer.as_deref());
    let username = rate_limit_username(&request.username);
    if !state.login_rate_limiter.allows(&source, &username).await {
        audit::record(
            &state,
            None,
            &headers,
            peer.as_deref(),
            "auth.bootstrap",
            Some("user"),
            None,
            AuditOutcome::Failure,
        )
        .await?;
        return Err(ApiError::AuthenticationRateLimited);
    }
    if !state.auth.bootstrap_required().await? {
        audit::record(
            &state,
            None,
            &headers,
            peer.as_deref(),
            "auth.bootstrap",
            Some("user"),
            None,
            AuditOutcome::Failure,
        )
        .await?;
        return Err(AuthError::AlreadyBootstrapped.into());
    }
    let bootstrap_secret = match headers
        .get("X-Ignitify-Bootstrap-Secret")
        .and_then(|value| value.to_str().ok())
    {
        Some(secret) => secret,
        None => {
            state
                .login_rate_limiter
                .record_failure(&source, &username)
                .await;
            audit::record(
                &state,
                None,
                &headers,
                peer.as_deref(),
                "auth.bootstrap",
                Some("user"),
                None,
                AuditOutcome::Failure,
            )
            .await?;
            return Err(AuthError::BootstrapUnavailable.into());
        }
    };
    let session = match state
        .auth
        .bootstrap_admin(bootstrap_secret, &request.username, &request.password)
        .await
    {
        Ok(session) => session,
        Err(error) => {
            if is_rate_limited_failure(&error) {
                state
                    .login_rate_limiter
                    .record_failure(&source, &username)
                    .await;
            }
            audit::record(
                &state,
                None,
                &headers,
                peer.as_deref(),
                "auth.bootstrap",
                Some("user"),
                None,
                AuditOutcome::Failure,
            )
            .await?;
            return Err(error.into());
        }
    };
    audit::record(
        &state,
        Some(&session.user),
        &headers,
        peer.as_deref(),
        "auth.bootstrap",
        Some("user"),
        Some(&session.user.id),
        AuditOutcome::Success,
    )
    .await?;
    session_response(state.secure_cookies, session)
}

pub(crate) async fn login(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Response, ApiError> {
    require_same_origin_request(&state, &headers)?;
    let source = rate_limit_source(&state, &headers, peer.as_deref());
    let username = rate_limit_username(&request.username);
    if !state.login_rate_limiter.allows(&source, &username).await {
        audit::record(
            &state,
            None,
            &headers,
            peer.as_deref(),
            "auth.login",
            Some("user"),
            None,
            AuditOutcome::Failure,
        )
        .await?;
        return Err(ApiError::AuthenticationRateLimited);
    }
    let session = match state.auth.login(&request.username, &request.password).await {
        Ok(session) => session,
        Err(error) => {
            if is_rate_limited_failure(&error) {
                state
                    .login_rate_limiter
                    .record_failure(&source, &username)
                    .await;
            }
            audit::record(
                &state,
                None,
                &headers,
                peer.as_deref(),
                "auth.login",
                Some("user"),
                None,
                AuditOutcome::Failure,
            )
            .await?;
            return Err(error.into());
        }
    };
    audit::record(
        &state,
        Some(&session.user),
        &headers,
        peer.as_deref(),
        "auth.login",
        Some("user"),
        Some(&session.user.id),
        AuditOutcome::Success,
    )
    .await?;
    session_response(state.secure_cookies, session)
}

pub(crate) async fn refresh(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    require_same_origin_request(&state, &headers)?;
    let token = match refresh_cookie(&headers) {
        Some(token) => token,
        None => {
            audit::record(
                &state,
                None,
                &headers,
                peer.as_deref(),
                "auth.refresh",
                Some("session"),
                None,
                AuditOutcome::Failure,
            )
            .await?;
            return Err(AuthError::InvalidToken.into());
        }
    };
    let session = match state.auth.refresh_session(token).await {
        Ok(session) => session,
        Err(error) => {
            audit::record(
                &state,
                None,
                &headers,
                peer.as_deref(),
                "auth.refresh",
                Some("session"),
                None,
                AuditOutcome::Failure,
            )
            .await?;
            return Err(error.into());
        }
    };
    audit::record(
        &state,
        Some(&session.user),
        &headers,
        peer.as_deref(),
        "auth.refresh",
        Some("session"),
        session.user.session_family_id.as_deref(),
        AuditOutcome::Success,
    )
    .await?;
    session_response(state.secure_cookies, session)
}

pub(crate) async fn logout(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    require_same_origin_request(&state, &headers)?;
    let actor = require_actor(&state, &headers).await.ok();
    if let Some(token) = refresh_cookie(&headers) {
        state.auth.revoke_refresh_token(token).await?;
    }
    audit::record(
        &state,
        actor.as_ref(),
        &headers,
        peer.as_deref(),
        "auth.logout",
        Some("session"),
        actor
            .as_ref()
            .and_then(|user| user.session_family_id.as_deref()),
        AuditOutcome::Success,
    )
    .await?;
    Ok(with_refresh_cookie(
        Json(MessageResponse {
            message: "logged out",
        })
        .into_response(),
        &clear_refresh_cookie(state.secure_cookies),
    ))
}

pub(crate) async fn step_up(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    Json(request): Json<StepUpRequest>,
) -> Result<Json<StepUpSession>, ApiError> {
    require_same_origin_request(&state, &headers)?;
    let actor = require_actor(&state, &headers).await?;
    if !actor.has_platform_operator_access() {
        return Err(ApiError::Forbidden);
    }
    let source = rate_limit_source(&state, &headers, peer.as_deref());
    let username = rate_limit_username(&actor.username);
    if !state.login_rate_limiter.allows(&source, &username).await {
        audit::record(
            &state,
            Some(&actor),
            &headers,
            peer.as_deref(),
            "auth.step_up",
            Some("session"),
            actor.session_family_id.as_deref(),
            AuditOutcome::Failure,
        )
        .await?;
        return Err(ApiError::AuthenticationRateLimited);
    }
    let session = match state
        .auth
        .create_step_up_session(&actor, &request.password)
        .await
    {
        Ok(session) => session,
        Err(error) => {
            state
                .login_rate_limiter
                .record_failure(&source, &username)
                .await;
            audit::record(
                &state,
                Some(&actor),
                &headers,
                peer.as_deref(),
                "auth.step_up",
                Some("session"),
                actor.session_family_id.as_deref(),
                AuditOutcome::Failure,
            )
            .await?;
            return Err(error.into());
        }
    };
    audit::record(
        &state,
        Some(&actor),
        &headers,
        peer.as_deref(),
        "auth.step_up",
        Some("session"),
        actor.session_family_id.as_deref(),
        AuditOutcome::Success,
    )
    .await?;
    Ok(Json(session))
}

pub(crate) async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ignitify_auth::AuthenticatedUser>, ApiError> {
    Ok(Json(require_actor(&state, &headers).await?))
}

fn rate_limit_source(
    state: &AppState,
    headers: &HeaderMap,
    peer: Option<&ConnectInfo<SocketAddr>>,
) -> String {
    audit::source_ip(state, headers, peer).unwrap_or_else(|| "unknown".to_owned())
}

fn rate_limit_username(username: &str) -> String {
    username.trim().to_ascii_lowercase()
}

fn is_rate_limited_failure(error: &AuthError) -> bool {
    matches!(
        error,
        AuthError::BootstrapUnavailable | AuthError::InactiveUser | AuthError::InvalidCredentials
    )
}
