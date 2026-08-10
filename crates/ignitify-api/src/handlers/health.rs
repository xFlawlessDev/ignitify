use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub(crate) struct HealthResponse {
    database: &'static str,
    docker: &'static str,
    ingress: &'static str,
}

#[utoipa::path(
    get,
    path = "/health",
    tag = "Health",
    responses(
        (status = 200, description = "Control-plane component health", body = HealthResponse),
        (status = 503, description = "Database health check failed")
    )
)]
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
