use axum::{Json, extract::State, http::HeaderMap};
use serde::Serialize;

use crate::{error::ApiError, extract::require_actor, state::AppState};

#[derive(Debug, Serialize)]
pub(crate) struct RuntimeStatusResponse {
    database: &'static str,
    runtime: &'static str,
    worker: &'static str,
}

pub(crate) async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RuntimeStatusResponse>, ApiError> {
    require_actor(&state, &headers).await?;

    let (database, runtime, worker) = tokio::join!(
        state.database.ping(),
        state.runtime_health.ready(),
        state.worker_health.ready(),
    );

    Ok(Json(RuntimeStatusResponse {
        database: if database.is_ok() {
            "ready"
        } else {
            "unavailable"
        },
        runtime: if runtime { "ready" } else { "unavailable" },
        worker: if worker { "ready" } else { "unavailable" },
    }))
}
