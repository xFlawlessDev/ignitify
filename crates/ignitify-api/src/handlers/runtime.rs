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

#[derive(Debug, Serialize)]
pub(crate) struct RuntimeContainersResponse {
    containers: Option<Vec<RuntimeContainerResponse>>,
}

#[derive(Debug, Serialize)]
struct RuntimePortResponse {
    container_port: u16,
    host_ip: Option<String>,
    host_port: Option<u16>,
    protocol: String,
}
#[derive(Debug, Serialize)]
struct RuntimeContainerResponse {
    id: String,
    name: String,
    image: String,
    state: String,
    status: String,
    health: Option<String>,
    ports: Vec<RuntimePortResponse>,
    restart_count: i64,
    cpu_percentage: Option<f64>,
    memory_usage_bytes: Option<i64>,
    cpu_limit_nano_cpus: Option<i64>,
    memory_limit_bytes: Option<i64>,
    managed: bool,
}

impl From<ignitify_control_plane::RuntimeContainer> for RuntimeContainerResponse {
    fn from(container: ignitify_control_plane::RuntimeContainer) -> Self {
        Self {
            id: container.id,
            name: container.name,
            image: container.image,
            state: container.state,
            status: container.status,
            ports: container
                .ports
                .into_iter()
                .map(|port| RuntimePortResponse {
                    container_port: port.container_port,
                    host_ip: port.host_ip,
                    host_port: port.host_port,
                    protocol: port.protocol,
                })
                .collect(),
            health: container.health,
            restart_count: container.restart_count,
            cpu_percentage: container.cpu_percentage,
            memory_usage_bytes: container.memory_usage_bytes,
            cpu_limit_nano_cpus: container.cpu_limit_nano_cpus,
            memory_limit_bytes: container.memory_limit_bytes,
            managed: container.managed,
        }
    }
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

pub(crate) async fn containers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<RuntimeContainersResponse>, ApiError> {
    require_actor(&state, &headers).await?;

    Ok(Json(RuntimeContainersResponse {
        containers: state
            .runtime_health
            .container_inventory()
            .await
            .map(|containers| containers.into_iter().map(Into::into).collect()),
    }))
}
