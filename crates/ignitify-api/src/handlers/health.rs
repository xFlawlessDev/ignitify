use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub(crate) struct HealthResponse {
    database: &'static str,
    docker: &'static str,
}

pub(crate) async fn health(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, StatusCode> {
    state
        .database
        .ping()
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(HealthResponse {
        database: "ready",
        docker: if state.runtime_health.ready().await && state.worker_health.ready().await {
            "ready"
        } else {
            "unavailable"
        },
    }))
}
