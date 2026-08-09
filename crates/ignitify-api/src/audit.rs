use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

use axum::{
    extract::ConnectInfo,
    http::{HeaderMap, header},
};
use ignitify_auth::AuthenticatedUser;
use ignitify_db::{AuditContext, AuditOutcome};
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

#[expect(
    clippy::too_many_arguments,
    reason = "an audit record requires the request, actor, action, target, and outcome context"
)]
pub(crate) async fn record(
    state: &AppState,
    actor: Option<&AuthenticatedUser>,
    headers: &HeaderMap,
    peer: Option<&ConnectInfo<SocketAddr>>,
    action: &str,
    resource_type: Option<&str>,
    resource_id: Option<&str>,
    outcome: AuditOutcome,
) -> Result<(), ApiError> {
    let context = AuditContext {
        source_ip: source_ip(state, headers, peer),
        session_family_id: actor.and_then(|user| user.session_family_id.clone()),
        request_id: Some(request_id(headers)),
        user_agent: headers
            .get(header::USER_AGENT)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.chars().take(512).collect()),
        outcome,
    };
    state
        .database
        .users()
        .audit_event(
            actor.map(|user| user.id.as_str()),
            action,
            resource_type,
            resource_id,
            &context,
        )
        .await?;
    Ok(())
}

pub(crate) fn source_ip(
    state: &AppState,
    headers: &HeaderMap,
    peer: Option<&ConnectInfo<SocketAddr>>,
) -> Option<String> {
    if state.trust_proxy_headers {
        let forwarded = headers
            .get("X-Forwarded-For")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(',').next())
            .map(str::trim)
            .and_then(|value| IpAddr::from_str(value).ok());
        if let Some(address) = forwarded {
            return Some(address.to_string());
        }
    }
    peer.map(|address| address.0.ip().to_string())
}

fn request_id(headers: &HeaderMap) -> String {
    headers
        .get("X-Ignitify-Request-ID")
        .and_then(|value| value.to_str().ok())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        })
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}
