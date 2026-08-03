use axum::{Json, extract::State, http::HeaderMap};
use serde::Serialize;

use crate::{error::ApiError, extract::require_actor, state::AppState};

#[derive(Debug, Serialize)]
pub(crate) struct RuntimeStatusResponse {
    database: &'static str,
    runtime: &'static str,
    worker: &'static str,
    metrics: Option<RuntimeMetricsResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RuntimeMetricsResponse {
    containers: i64,
    containers_running: i64,
    images: i64,
    cpus: i64,
    memory_bytes: i64,
}

pub(crate) async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RuntimeStatusResponse>, ApiError> {
    require_actor(&state, &headers).await?;

    let (database, runtime, worker, metrics) = tokio::join!(
        state.database.ping(),
        state.runtime_health.ready(),
        state.worker_health.ready(),
        state.runtime_health.host_metrics(),
    );

    Ok(Json(RuntimeStatusResponse {
        database: if database.is_ok() {
            "ready"
        } else {
            "unavailable"
        },
        runtime: if runtime { "ready" } else { "unavailable" },
        worker: if worker { "ready" } else { "unavailable" },
        metrics: metrics.map(|metrics| RuntimeMetricsResponse {
            containers: metrics.containers,
            containers_running: metrics.containers_running,
            images: metrics.images,
            cpus: metrics.cpus,
            memory_bytes: metrics.memory_bytes,
        }),
    }))
}
