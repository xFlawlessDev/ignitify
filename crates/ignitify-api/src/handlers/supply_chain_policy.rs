use axum::{
    Json,
    extract::{ConnectInfo, Extension, State},
    http::HeaderMap,
};
use ignitify_db::AuditOutcome;
use ignitify_domain::{SupplyChainEnforcement, SupplyChainPolicy};
use serde::{Deserialize, Serialize};

use crate::{
    audit,
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SupplyChainPolicyRequest {
    enforcement: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct SupplyChainPolicyResponse {
    enforcement: SupplyChainEnforcement,
    updated_at: String,
}

impl From<SupplyChainPolicy> for SupplyChainPolicyResponse {
    fn from(policy: SupplyChainPolicy) -> Self {
        Self {
            enforcement: policy.enforcement,
            updated_at: policy.updated_at,
        }
    }
}

pub(crate) async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SupplyChainPolicyResponse>, ApiError> {
    require_platform_operator(&state, &headers).await?;
    let policy = state.database.deployments().supply_chain_policy().await?;
    Ok(Json(policy.into()))
}

pub(crate) async fn update(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    Json(request): Json<SupplyChainPolicyRequest>,
) -> Result<Json<SupplyChainPolicyResponse>, ApiError> {
    let actor = require_platform_operator(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let enforcement = request
        .enforcement
        .trim()
        .to_ascii_lowercase()
        .as_str()
        .try_into()
        .map_err(|_| ApiError::BadRequest("supply-chain enforcement mode is invalid"))?;
    let policy = state
        .database
        .deployments()
        .update_supply_chain_enforcement(enforcement)
        .await?;
    audit::record(
        &state,
        Some(&actor),
        &headers,
        peer.as_deref(),
        "supply_chain_policy.update",
        Some("supply_chain_policy"),
        Some("global"),
        AuditOutcome::Success,
    )
    .await?;
    if let Some(control) = &state.control {
        let _ = control.wake_worker();
    }
    Ok(Json(policy.into()))
}

async fn require_platform_operator(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<ignitify_auth::AuthenticatedUser, ApiError> {
    let actor = require_actor(state, headers).await?;
    if actor.has_platform_operator_access() {
        Ok(actor)
    } else {
        Err(ApiError::Forbidden)
    }
}
