//! Docker Engine adapter for restricted Ignitify image containers.

mod actions;

pub use actions::{
    ContainerConfig, ContainerDetails, ContainerMount, ContainerNetwork, ContainerTerminalEvent,
    ContainerTerminalSession,
};

use std::{collections::HashMap, env, path::PathBuf};

use futures_util::{StreamExt, TryStreamExt};

use bollard::{
    Docker,
    container::{
        Config, CreateContainerOptions, ListContainersOptions, RemoveContainerOptions,
        StartContainerOptions, StatsOptions, StopContainerOptions,
    },
    image::CreateImageOptions,
    models::{HealthConfig, HealthStatusEnum, HostConfig},
};
use ignitify_control_plane::{
    Error as ControlError, HostRuntimeMetrics, ImageRuntime, IngressRoute, RuntimeContainer,
    RuntimeDeployment, RuntimeHealth, RuntimeLog, RuntimeObservation, RuntimePort,
};
use thiserror::Error;

const MANAGED_LABEL: &str = "com.ignitify.managed";
const SERVICE_LABEL: &str = "com.ignitify.service-id";
const GENERATION_LABEL: &str = "com.ignitify.generation";
const MEMORY_LIMIT_BYTES: i64 = 512 * 1024 * 1024;
const NANO_CPUS: i64 = 1_000_000_000;
const PID_LIMIT: i64 = 256;

const MAX_CONCURRENT_CONTAINER_OBSERVATIONS: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct RuntimeMetrics {
    pub containers: i64,
    pub containers_running: i64,
    pub images: i64,
    pub cpus: i64,
    pub memory_bytes: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerDiskUsage {
    pub used_bytes: u64,
    pub root_dir: Option<PathBuf>,
}

#[derive(Clone)]
pub struct DockerRuntime {
    docker: Docker,
}

impl DockerRuntime {
    pub fn from_environment() -> Result<Self> {
        let docker = match env::var("IGNITIFY_DOCKER_HOST") {
            Ok(host) if host.starts_with("tcp://") || host.starts_with("http://") => {
                Docker::connect_with_http(&host, 120, bollard::API_DEFAULT_VERSION)?
            }
            Ok(host) if host.starts_with("unix://") => Docker::connect_with_socket(
                host.trim_start_matches("unix://"),
                120,
                bollard::API_DEFAULT_VERSION,
            )?,
            Ok(_) => Docker::connect_with_local_defaults()?,
            Err(_) => Docker::connect_with_local_defaults()?,
        };
        Ok(Self { docker })
    }

    pub async fn ping(&self) -> Result<()> {
        self.docker.ping().await?;
        Ok(())
    }

    pub async fn network_exists(&self, name: &str) -> Result<bool> {
        match self.docker.inspect_network::<String>(name, None).await {
            Ok(_) => Ok(true),
            Err(error) if is_not_found(&error) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn has_running_container_with_label(&self, label: &str) -> Result<bool> {
        let filters = HashMap::from([("label".to_owned(), vec![label.to_owned()])]);
        let containers = self
            .docker
            .list_containers(Some(ListContainersOptions::<String> {
                filters,
                ..Default::default()
            }))
            .await?;
        Ok(!containers.is_empty())
    }

    pub async fn metrics(&self) -> Result<RuntimeMetrics> {
        let info = self.docker.info().await?;
        Ok(RuntimeMetrics {
            containers: info.containers.unwrap_or_default(),
            containers_running: info.containers_running.unwrap_or_default(),
            images: info.images.unwrap_or_default(),
            cpus: info.ncpu.unwrap_or_default(),
            memory_bytes: info.mem_total.unwrap_or_default(),
        })
    }

    pub async fn disk_usage(&self) -> Result<DockerDiskUsage> {
        let usage = self.docker.df().await?;
        let info = self.docker.info().await?;
        let layers = usage.layers_size.and_then(|size| u64::try_from(size).ok());
        let containers = usage
            .containers
            .unwrap_or_default()
            .into_iter()
            .filter_map(|container| container.size_rw)
            .filter_map(|size| u64::try_from(size).ok())
            .sum::<u64>();
        let volumes = usage
            .volumes
            .unwrap_or_default()
            .into_iter()
            .filter_map(|volume| volume.usage_data)
            .filter_map(|usage| u64::try_from(usage.size).ok())
            .sum::<u64>();
        let build_cache = usage
            .build_cache
            .unwrap_or_default()
            .into_iter()
            .filter_map(|cache| cache.size)
            .filter_map(|size| u64::try_from(size).ok())
            .sum::<u64>();

        Ok(DockerDiskUsage {
            used_bytes: layers.unwrap_or_default() + containers + volumes + build_cache,
            root_dir: info.docker_root_dir.map(PathBuf::from),
        })
    }

    pub async fn containers(&self) -> Result<Vec<RuntimeContainer>> {
        let mut containers = futures_util::stream::iter(
            self.docker
                .list_containers(Some(ListContainersOptions::<String> {
                    all: true,
                    ..Default::default()
                }))
                .await?,
        )
        .map(|summary| async move { self.observe_container(summary).await })
        .buffer_unordered(MAX_CONCURRENT_CONTAINER_OBSERVATIONS)
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        containers.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(containers)
    }

    async fn observe_container(
        &self,
        summary: bollard::models::ContainerSummary,
    ) -> Result<Option<RuntimeContainer>> {
        let Some(id) = summary.id else {
            return Ok(None);
        };
        let fallback_name = summary
            .names
            .as_ref()
            .and_then(|names| names.first())
            .cloned()
            .unwrap_or_else(|| id.chars().take(12).collect());
        let mut ports = summary
            .ports
            .unwrap_or_default()
            .into_iter()
            .map(|port| RuntimePort {
                container_port: port.private_port,
                host_ip: port.ip.filter(|ip| !ip.is_empty()),
                host_port: port.public_port,
                protocol: port
                    .typ
                    .map(|protocol| protocol.to_string())
                    .filter(|protocol| !protocol.is_empty())
                    .unwrap_or_else(|| "tcp".to_owned()),
            })
            .collect::<Vec<_>>();
        ports.sort_by(|left, right| {
            (left.container_port, &left.protocol, left.host_port).cmp(&(
                right.container_port,
                &right.protocol,
                right.host_port,
            ))
        });
        let fallback_state = summary.state.unwrap_or_default();
        let status = summary.status.unwrap_or_else(|| fallback_state.clone());
        let inspected = match self.docker.inspect_container(&id, None).await {
            Ok(inspected) => inspected,
            Err(error) if is_not_found(&error) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let state = inspected
            .state
            .as_ref()
            .and_then(|state| state.status)
            .map(|status| status.to_string())
            .unwrap_or(fallback_state);
        let health = inspected
            .state
            .as_ref()
            .and_then(|state| state.health.as_ref())
            .and_then(|health| health.status)
            .map(|status| status.to_string())
            .filter(|status| !status.is_empty() && status != "none");
        let (cpu_percentage, memory_usage_bytes) = if state == "running" {
            self.container_stats(&id).await
        } else {
            (None, None)
        };
        let host_config = inspected.host_config.as_ref();
        let managed = inspected
            .config
            .as_ref()
            .and_then(|config| config.labels.as_ref())
            .and_then(|labels| labels.get(MANAGED_LABEL))
            .is_some_and(|value| value == "true");

        Ok(Some(RuntimeContainer {
            id,
            name: inspected
                .name
                .unwrap_or(fallback_name)
                .trim_start_matches('/')
                .to_owned(),
            image: summary.image.unwrap_or_default(),
            state,
            status,
            health,
            ports,
            restart_count: inspected.restart_count.unwrap_or_default(),
            cpu_percentage,
            memory_usage_bytes,
            cpu_limit_nano_cpus: host_config.and_then(|config| config.nano_cpus),
            memory_limit_bytes: host_config.and_then(|config| config.memory),
            managed,
        }))
    }

    #[cfg(test)]
    fn docker(&self) -> &Docker {
        &self.docker
    }

    fn container_name(deployment: &RuntimeDeployment) -> String {
        format!(
            "ignitify-svc-{}-g{}",
            deployment.service_id, deployment.generation
        )
    }

    async fn container_stats(&self, id: &str) -> (Option<f64>, Option<i64>) {
        let mut stats = self.docker.stats(
            id,
            Some(StatsOptions {
                stream: false,
                one_shot: false,
            }),
        );
        let Some(Ok(stats)) = stats.next().await else {
            return (None, None);
        };

        let cpu_delta = stats
            .cpu_stats
            .cpu_usage
            .total_usage
            .saturating_sub(stats.precpu_stats.cpu_usage.total_usage);
        let system_delta = stats
            .cpu_stats
            .system_cpu_usage
            .unwrap_or_default()
            .saturating_sub(stats.precpu_stats.system_cpu_usage.unwrap_or_default());
        let cpu_count = stats
            .cpu_stats
            .online_cpus
            .or_else(|| {
                stats
                    .cpu_stats
                    .cpu_usage
                    .percpu_usage
                    .as_ref()
                    .and_then(|cpus| u64::try_from(cpus.len()).ok())
            })
            .unwrap_or(1);
        let cpu_percentage = (cpu_delta > 0 && system_delta > 0)
            .then(|| (cpu_delta as f64 / system_delta as f64) * cpu_count as f64 * 100.0);
        let memory_usage_bytes = stats
            .memory_stats
            .usage
            .and_then(|usage| i64::try_from(usage).ok());

        (cpu_percentage, memory_usage_bytes)
    }
}

impl RuntimeHealth for DockerRuntime {
    fn ready(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
        Box::pin(async move { self.ping().await.is_ok() })
    }

    fn host_metrics(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<HostRuntimeMetrics>> + Send + '_>>
    {
        Box::pin(async move {
            self.metrics().await.ok().map(|metrics| HostRuntimeMetrics {
                containers: metrics.containers,
                containers_running: metrics.containers_running,
                images: metrics.images,
                cpus: metrics.cpus,
                memory_bytes: metrics.memory_bytes,
            })
        })
    }

    fn container_inventory(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Option<Vec<RuntimeContainer>>> + Send + '_>,
    > {
        Box::pin(async move { self.containers().await.ok() })
    }
}

impl ImageRuntime for DockerRuntime {
    fn runtime_ref(&self, deployment: &RuntimeDeployment) -> String {
        Self::container_name(deployment)
    }

    async fn start(
        &self,
        deployment: &RuntimeDeployment,
        environment: Vec<String>,
    ) -> std::result::Result<String, ControlError> {
        self.start_with_routes(deployment, environment, Vec::new())
            .await
    }

    async fn reconcile_routes(
        &self,
        deployment: &RuntimeDeployment,
        runtime_ref: &str,
        environment: Vec<String>,
        routes: Vec<IngressRoute>,
    ) -> std::result::Result<bool, ControlError> {
        let observation = self.inspect(deployment, runtime_ref).await?;
        if !observation.owned {
            return Ok(false);
        }
        let inspected = self
            .docker
            .inspect_container(runtime_ref, None)
            .await
            .map_err(|_| ControlError::Runtime)?;
        if routes_match(&inspected, &routes) {
            return Ok(true);
        }
        self.stop_and_remove_owned(
            runtime_ref,
            &deployment.service_id.to_string(),
            deployment.generation,
        )
        .await
        .map_err(|_| ControlError::Runtime)?;
        self.start_with_routes(deployment, environment, routes)
            .await?;
        Ok(true)
    }

    async fn inspect(
        &self,
        deployment: &RuntimeDeployment,
        runtime_ref: &str,
    ) -> std::result::Result<RuntimeObservation, ControlError> {
        let inspected = match self.docker.inspect_container(runtime_ref, None).await {
            Ok(inspected) => inspected,
            Err(error) if is_not_found(&error) => {
                return Ok(RuntimeObservation {
                    owned: true,
                    running: false,
                    healthy: None,
                    health_failing: false,
                });
            }
            Err(_) => return Err(ControlError::Runtime),
        };
        let owned = has_expected_labels(
            &inspected,
            &deployment.service_id.to_string(),
            deployment.generation,
        );
        let state = inspected.state;
        let health_status = state
            .as_ref()
            .and_then(|state| state.health.as_ref())
            .and_then(|health| health.status);
        Ok(RuntimeObservation {
            owned,
            running: state
                .as_ref()
                .and_then(|state| state.running)
                .unwrap_or(false),
            healthy: health_status.map(|status| status == HealthStatusEnum::HEALTHY),
            health_failing: health_status == Some(HealthStatusEnum::UNHEALTHY),
        })
    }

    async fn stop(
        &self,
        runtime_ref: &str,
        service_id: &str,
        generation: i64,
    ) -> std::result::Result<bool, ControlError> {
        self.stop_and_remove_owned(runtime_ref, service_id, generation)
            .await
            .map_err(|_| ControlError::Runtime)
    }

    async fn logs(
        &self,
        runtime_ref: &str,
        since: i64,
    ) -> std::result::Result<Vec<RuntimeLog>, ControlError> {
        let mut logs = self.docker.logs(
            runtime_ref,
            Some(bollard::container::LogsOptions::<String> {
                follow: false,
                stdout: true,
                stderr: true,
                since,
                until: 0,
                timestamps: false,
                tail: "all".to_owned(),
            }),
        );
        let mut records = Vec::new();
        while let Some(log) = logs.next().await {
            let log = log.map_err(|_| ControlError::Runtime)?;
            let stream = match log {
                bollard::container::LogOutput::StdOut { .. } => "stdout",
                bollard::container::LogOutput::StdErr { .. } => "stderr",
                bollard::container::LogOutput::StdIn { .. }
                | bollard::container::LogOutput::Console { .. } => "system",
            };
            for line in log.to_string().lines() {
                records.push(RuntimeLog {
                    stream: stream.to_owned(),
                    line: line.to_owned(),
                });
            }
        }
        Ok(records)
    }
}

impl DockerRuntime {
    async fn start_with_routes(
        &self,
        deployment: &RuntimeDeployment,
        environment: Vec<String>,
        routes: Vec<IngressRoute>,
    ) -> std::result::Result<String, ControlError> {
        let bollard = &self.docker;
        let ignitify_domain::ServiceSpec::Image {
            image_reference,
            healthcheck,
            ..
        } = &deployment.spec
        else {
            return Err(ControlError::Runtime);
        };
        let runtime_image = deployment
            .local_image_id
            .as_deref()
            .unwrap_or(image_reference);
        if deployment.local_image_id.is_none() {
            let mut pull = bollard.create_image(
                Some(CreateImageOptions {
                    from_image: image_reference.as_str(),
                    ..Default::default()
                }),
                None,
                None,
            );
            while pull
                .try_next()
                .await
                .map_err(|_| ControlError::Runtime)?
                .is_some()
            {}
        }
        let name = Self::container_name(deployment);
        let mut labels = HashMap::from([
            (MANAGED_LABEL.to_owned(), "true".to_owned()),
            (SERVICE_LABEL.to_owned(), deployment.service_id.to_string()),
            (
                GENERATION_LABEL.to_owned(),
                deployment.generation.to_string(),
            ),
        ]);
        let network = routes.first().map(|route| route.network.clone());
        for route in routes {
            labels.extend(route.labels);
        }
        let runtime_ref = match bollard
            .create_container(
                Some(CreateContainerOptions::<String> {
                    name: name.clone(),
                    platform: None,
                }),
                Config {
                    image: Some(runtime_image.to_owned()),
                    env: (!environment.is_empty()).then_some(environment),
                    healthcheck: healthcheck.as_ref().map(|argv| HealthConfig {
                        test: Some(
                            std::iter::once("CMD".to_owned())
                                .chain(argv.iter().cloned())
                                .collect(),
                        ),
                        ..Default::default()
                    }),
                    labels: Some(labels),
                    host_config: Some(HostConfig {
                        auto_remove: Some(false),
                        privileged: Some(false),
                        network_mode: Some(network.unwrap_or_else(|| "none".to_owned())),
                        readonly_rootfs: Some(false),
                        memory: Some(MEMORY_LIMIT_BYTES),
                        nano_cpus: Some(NANO_CPUS),
                        pids_limit: Some(PID_LIMIT),
                        security_opt: Some(vec!["no-new-privileges:true".to_owned()]),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .await
        {
            Ok(_) => name.clone(),
            Err(error) if is_conflict(&error) => {
                if !self
                    .is_owned(
                        &name,
                        &deployment.service_id.to_string(),
                        deployment.generation,
                    )
                    .await
                    .map_err(|_| ControlError::Runtime)?
                {
                    return Err(ControlError::Runtime);
                }
                name
            }
            Err(_) => return Err(ControlError::Runtime),
        };
        match bollard
            .start_container(&runtime_ref, None::<StartContainerOptions<String>>)
            .await
        {
            Ok(()) => Ok(runtime_ref),
            Err(error) if is_already_running(&error) => Ok(runtime_ref),
            Err(_) => Err(ControlError::Runtime),
        }
    }

    async fn is_owned(&self, runtime_ref: &str, service_id: &str, generation: i64) -> Result<bool> {
        let inspected = match self.docker.inspect_container(runtime_ref, None).await {
            Ok(inspected) => inspected,
            Err(error) if is_not_found(&error) => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        Ok(has_expected_labels(&inspected, service_id, generation))
    }

    pub async fn stop_and_remove_owned(
        &self,
        runtime_ref: &str,
        service_id: &str,
        generation: i64,
    ) -> Result<bool> {
        let inspected = match self.docker.inspect_container(runtime_ref, None).await {
            Ok(inspected) => inspected,
            Err(error) if is_not_found(&error) => return Ok(true),
            Err(error) => return Err(error.into()),
        };
        if !has_expected_labels(&inspected, service_id, generation) {
            return Ok(false);
        }
        match self
            .docker
            .stop_container(runtime_ref, None::<StopContainerOptions>)
            .await
        {
            Ok(()) => {}
            Err(error) if is_not_found_or_stopped(&error) => {}
            Err(error) => return Err(error.into()),
        }
        match self
            .docker
            .remove_container(
                runtime_ref,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
        {
            Ok(()) => Ok(true),
            Err(error) if is_not_found(&error) => Ok(true),
            Err(error) => Err(error.into()),
        }
    }
}

fn routes_match(
    inspected: &bollard::models::ContainerInspectResponse,
    routes: &[IngressRoute],
) -> bool {
    let labels = inspected
        .config
        .as_ref()
        .and_then(|config| config.labels.as_ref());
    let mut expected = routes.iter().flat_map(|route| route.labels.iter());
    let network = routes
        .first()
        .map(|route| route.network.as_str())
        .unwrap_or("none");
    let has_only_expected_traefik_labels = labels.is_some_and(|labels| {
        labels
            .keys()
            .filter(|key| key.starts_with("traefik."))
            .count()
            == routes.iter().map(|route| route.labels.len()).sum::<usize>()
            && expected.all(|(key, value)| labels.get(key) == Some(value))
    });
    inspected
        .host_config
        .as_ref()
        .and_then(|config| config.network_mode.as_deref())
        == Some(network)
        && has_only_expected_traefik_labels
}

fn has_expected_labels(
    inspected: &bollard::models::ContainerInspectResponse,
    service_id: &str,
    generation: i64,
) -> bool {
    let labels = inspected
        .config
        .as_ref()
        .and_then(|config| config.labels.as_ref());
    labels.is_some_and(|labels| {
        labels
            .get(MANAGED_LABEL)
            .is_some_and(|value| value == "true")
            && labels
                .get(SERVICE_LABEL)
                .is_some_and(|value| value == service_id)
            && labels
                .get(GENERATION_LABEL)
                .is_some_and(|value| value == &generation.to_string())
    })
}

fn is_not_found(error: &bollard::errors::Error) -> bool {
    matches!(
        error,
        bollard::errors::Error::DockerResponseServerError {
            status_code: 404,
            ..
        }
    )
}

fn is_conflict(error: &bollard::errors::Error) -> bool {
    matches!(
        error,
        bollard::errors::Error::DockerResponseServerError {
            status_code: 409,
            ..
        }
    )
}

fn is_already_running(error: &bollard::errors::Error) -> bool {
    matches!(
        error,
        bollard::errors::Error::DockerResponseServerError {
            status_code: 304,
            ..
        }
    )
}

fn is_not_found_or_stopped(error: &bollard::errors::Error) -> bool {
    is_not_found(error)
        || matches!(
            error,
            bollard::errors::Error::DockerResponseServerError {
                status_code: 304,
                ..
            }
        )
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Docker connection failed")]
    Connection,
    #[error("container not found")]
    ContainerNotFound,
    #[error("container is not running")]
    ContainerNotRunning,
    #[error("invalid container reference")]
    InvalidContainerReference,
    #[error("invalid upload path")]
    InvalidUploadPath,
    #[error("container terminal is unavailable")]
    TerminalUnavailable,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Docker(#[from] bollard::errors::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use std::env;

    use super::{DockerRuntime, GENERATION_LABEL, MANAGED_LABEL, SERVICE_LABEL};
    use bollard::{
        container::{InspectContainerOptions, RemoveContainerOptions},
        models::HostConfig,
    };
    use ignitify_control_plane::ImageRuntime;
    use ignitify_domain::{DeploymentId, ServiceId, ServiceSpec};

    fn docker_test_enabled() -> bool {
        env::var("IGNITIFY_DOCKER_TEST").is_ok_and(|value| value == "1")
    }

    #[tokio::test]
    async fn deploys_restricted_digest_image_when_opted_in() {
        if !docker_test_enabled() {
            return;
        }
        let runtime = DockerRuntime::from_environment().unwrap();
        runtime.ping().await.unwrap();
        let service_id = ServiceId::new(uuid::Uuid::new_v4().to_string()).unwrap();
        let deployment = ignitify_control_plane::RuntimeDeployment {
            id: DeploymentId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
            service_id: service_id.clone(),
            generation: 1,
            spec: ServiceSpec::image(
                "caddy:2.11.4-alpine@sha256:98eb57d882ccd5213d1688764db10c1ca2c58a1ca3a6717a3411ad798f7a423a",
                Some(80),
                None,
            )
            .unwrap(),
            local_image_id: None,
        };
        let runtime_ref = runtime.start(&deployment, vec![]).await.unwrap();
        let result = async {
            let inspected = runtime
                .docker()
                .inspect_container(&runtime_ref, Some(InspectContainerOptions { size: false }))
                .await
                .unwrap();
            let labels = inspected.config.unwrap().labels.unwrap();
            let host = inspected.host_config.unwrap_or_else(HostConfig::default);
            assert_eq!(labels.get(MANAGED_LABEL), Some(&"true".to_owned()));
            assert_eq!(labels.get(SERVICE_LABEL), Some(&service_id.to_string()));
            assert_eq!(labels.get(GENERATION_LABEL), Some(&"1".to_owned()));
            assert!(
                host.port_bindings
                    .as_ref()
                    .is_none_or(|bindings| bindings.is_empty())
            );
            assert_eq!(host.network_mode.as_deref(), Some("none"));
            assert!(host.pid_mode.as_deref().is_none_or(str::is_empty));
            assert!(
                host.ipc_mode
                    .as_deref()
                    .is_none_or(|mode| mode == "private")
            );
            assert!(host.uts_mode.as_deref().is_none_or(str::is_empty));
            assert_eq!(host.privileged, Some(false));
            assert_eq!(host.pids_limit, Some(256));
            runtime
                .docker()
                .remove_container(
                    &runtime_ref,
                    Some(RemoveContainerOptions {
                        force: true,
                        ..Default::default()
                    }),
                )
                .await
                .unwrap();
        };
        result.await;
    }
}
