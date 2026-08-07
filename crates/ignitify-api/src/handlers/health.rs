use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub(crate) struct HealthResponse {
    database: &'static str,
    docker: &'static str,
    ingress: &'static str,
}

pub(crate) async fn health(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, StatusCode> {
    state
        .database
        .ping()
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let (runtime, worker, ingress) = tokio::join!(
        state.runtime_health.ready(),
        state.worker_health.ready(),
        state.ingress_health.ready(),
    );
    Ok(Json(HealthResponse {
        database: "ready",
        docker: if runtime && worker {
            "ready"
        } else {
            "unavailable"
        },
        ingress: if ingress { "ready" } else { "unavailable" },
    }))
}
