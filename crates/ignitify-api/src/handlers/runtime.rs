use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
};
use ignitify_runtime_docker::{
    ContainerConfig, ContainerDetails, ContainerMount, ContainerNetwork,
};
use serde::Serialize;

use crate::{
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

const MAX_UPLOAD_BYTES: usize = 8 * 1024 * 1024;

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
pub(crate) struct SystemMetricsResponse {
    cpu_usage_percentage: f64,
    cpu_cores: u32,
    memory_used_bytes: u64,
    memory_total_bytes: u64,
    disk_used_bytes: u64,
    disk_total_bytes: u64,
    docker_disk_used_bytes: Option<u64>,
    docker_disk_total_bytes: Option<u64>,
    block_read_bytes_per_second: f64,
    block_write_bytes_per_second: f64,
    network_receive_bytes_per_second: f64,
    network_transmit_bytes_per_second: f64,
}

impl From<ignitify_control_plane::SystemMetricsSnapshot> for SystemMetricsResponse {
    fn from(metrics: ignitify_control_plane::SystemMetricsSnapshot) -> Self {
        Self {
            cpu_usage_percentage: metrics.cpu_usage_percentage,
            cpu_cores: metrics.cpu_cores,
            memory_used_bytes: metrics.memory_used_bytes,
            memory_total_bytes: metrics.memory_total_bytes,
            disk_used_bytes: metrics.disk_used_bytes,
            disk_total_bytes: metrics.disk_total_bytes,
            docker_disk_used_bytes: metrics.docker_disk_used_bytes,
            docker_disk_total_bytes: metrics.docker_disk_total_bytes,
            block_read_bytes_per_second: metrics.block_read_bytes_per_second,
            block_write_bytes_per_second: metrics.block_write_bytes_per_second,
            network_receive_bytes_per_second: metrics.network_receive_bytes_per_second,
            network_transmit_bytes_per_second: metrics.network_transmit_bytes_per_second,
        }
    }
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

#[derive(Debug, Serialize)]
pub(crate) struct ContainerDetailsResponse {
    id: String,
    name: String,
    image: String,
    state: String,
    status: String,
    config: ContainerConfigResponse,
    mounts: Vec<ContainerMountResponse>,
    networks: Vec<ContainerNetworkResponse>,
}

#[derive(Debug, Serialize)]
struct ContainerConfigResponse {
    command: Vec<String>,
    entrypoint: Vec<String>,
    user: Option<String>,
    working_dir: Option<String>,
    tty: bool,
    environment_keys: Vec<String>,
    labels: Vec<ContainerLabelResponse>,
    restart_policy: Option<String>,
    privileged: bool,
}

#[derive(Debug, Serialize)]
struct ContainerLabelResponse {
    key: String,
    value: String,
}

#[derive(Debug, Serialize)]
struct ContainerMountResponse {
    kind: String,
    source: Option<String>,
    destination: Option<String>,
    read_only: bool,
}

#[derive(Debug, Serialize)]
struct ContainerNetworkResponse {
    name: String,
    ip_address: Option<String>,
    gateway: Option<String>,
    mac_address: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ContainerLogsResponse {
    logs: String,
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

impl From<ContainerDetails> for ContainerDetailsResponse {
    fn from(details: ContainerDetails) -> Self {
        Self {
            id: details.id,
            name: details.name,
            image: details.image,
            state: details.state,
            status: details.status,
            config: details.config.into(),
            mounts: details.mounts.into_iter().map(Into::into).collect(),
            networks: details.networks.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<ContainerConfig> for ContainerConfigResponse {
    fn from(config: ContainerConfig) -> Self {
        Self {
            command: config.command,
            entrypoint: config.entrypoint,
            user: config.user,
            working_dir: config.working_dir,
            tty: config.tty,
            environment_keys: config.environment_keys,
            labels: config
                .labels
                .into_iter()
                .map(|(key, value)| ContainerLabelResponse { key, value })
                .collect(),
            restart_policy: config.restart_policy,
            privileged: config.privileged,
        }
    }
}

impl From<ContainerMount> for ContainerMountResponse {
    fn from(mount: ContainerMount) -> Self {
        Self {
            kind: mount.kind,
            source: mount.source,
            destination: mount.destination,
            read_only: mount.read_only,
        }
    }
}

impl From<ContainerNetwork> for ContainerNetworkResponse {
    fn from(network: ContainerNetwork) -> Self {
        Self {
            name: network.name,
            ip_address: network.ip_address,
            gateway: network.gateway,
            mac_address: network.mac_address,
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

pub(crate) async fn metrics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SystemMetricsResponse>, ApiError> {
    require_actor(&state, &headers).await?;

    state
        .system_metrics
        .metrics()
        .await
        .map(SystemMetricsResponse::from)
        .map(Json)
        .ok_or(ApiError::CapabilityUnavailable)
}

pub(crate) async fn container_details(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(container_id): Path<String>,
) -> Result<Json<ContainerDetailsResponse>, ApiError> {
    require_container_admin(&state, &headers).await?;
    let details = state
        .docker_runtime()?
        .container_details(&container_id)
        .await?;
    Ok(Json(details.into()))
}

pub(crate) async fn container_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(container_id): Path<String>,
) -> Result<Json<ContainerLogsResponse>, ApiError> {
    require_container_admin(&state, &headers).await?;
    let logs = state
        .docker_runtime()?
        .container_logs(&container_id)
        .await?;
    Ok(Json(ContainerLogsResponse { logs }))
}

pub(crate) async fn upload_container_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(container_id): Path<String>,
    mut multipart: Multipart,
) -> Result<StatusCode, ApiError> {
    require_container_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;

    let mut destination = "/tmp".to_owned();
    let mut file = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::BadRequest("invalid file upload"))?
    {
        let name = field.name().map(str::to_owned);
        match name.as_deref() {
            Some("destination") => {
                destination = field
                    .text()
                    .await
                    .map_err(|_| ApiError::BadRequest("invalid upload destination"))?;
            }
            Some("file") if file.is_none() => {
                let file_name = field
                    .file_name()
                    .map(str::to_owned)
                    .ok_or(ApiError::BadRequest("uploaded file must have a name"))?;
                let data = field
                    .bytes()
                    .await
                    .map_err(|_| ApiError::BadRequest("invalid file upload"))?;
                if data.len() > MAX_UPLOAD_BYTES {
                    return Err(ApiError::BadRequest("uploaded file is too large"));
                }
                file = Some((file_name, data));
            }
            _ => {}
        }
    }

    let (file_name, data) = file.ok_or(ApiError::BadRequest("uploaded file is required"))?;
    state
        .docker_runtime()?
        .upload_file(&container_id, &destination, &file_name, &data)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn remove_container(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(container_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_container_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    state
        .docker_runtime()?
        .remove_container(&container_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn require_container_admin(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(), ApiError> {
    if require_actor(state, headers).await?.has_admin_access() {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}
