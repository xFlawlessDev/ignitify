use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use ignitify_db::ServiceActor;
use serde::Serialize;

use crate::{error::ApiError, extract::require_actor, state::AppState};

#[derive(Debug, Serialize)]
pub(crate) struct TerminalCapabilityResponse {
    available: bool,
    reason: &'static str,
}

pub(crate) async fn capability(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(service_id): Path<String>,
) -> Result<Json<TerminalCapabilityResponse>, ApiError> {
    let actor = require_actor(&state, &headers).await?;
    let service = state
        .services
        .get(
            ServiceActor {
                id: &actor.id,
                is_admin: actor.has_admin_access(),
            },
            &service_id,
        )
        .await?
        .ok_or(ApiError::NotFound)?;
    if !actor.has_admin_access() && service.role != "owner" && service.role != "editor" {
        return Err(ApiError::Forbidden);
    }
    Ok(Json(TerminalCapabilityResponse {
        available: false,
        reason: "runtime exec transport is not implemented",
    }))
}
