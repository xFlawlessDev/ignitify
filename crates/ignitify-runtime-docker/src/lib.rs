//! Docker Engine adapter for restricted Ignitify image containers.

use std::{collections::HashMap, env};

use futures_util::{StreamExt, TryStreamExt};

use bollard::{
    Docker,
    container::{
        Config, CreateContainerOptions, RemoveContainerOptions, StartContainerOptions,
        StopContainerOptions,
    },
    image::CreateImageOptions,
    models::{HealthConfig, HealthStatusEnum, HostConfig},
};
use ignitify_control_plane::{
    Error as ControlError, HostRuntimeMetrics, ImageRuntime, IngressRoute, RuntimeHealth,
    RuntimeObservation,
};
use ignitify_db::{DeploymentRecord, NewDeploymentLog};
use thiserror::Error;

const MANAGED_LABEL: &str = "com.ignitify.managed";
const SERVICE_LABEL: &str = "com.ignitify.service-id";
const GENERATION_LABEL: &str = "com.ignitify.generation";
const MEMORY_LIMIT_BYTES: i64 = 512 * 1024 * 1024;
const NANO_CPUS: i64 = 1_000_000_000;
const PID_LIMIT: i64 = 256;

#[derive(Debug, Clone, Copy)]
pub struct RuntimeMetrics {
    pub containers: i64,
    pub containers_running: i64,
    pub images: i64,
    pub cpus: i64,
    pub memory_bytes: i64,
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

    #[cfg(test)]
    fn docker(&self) -> &Docker {
        &self.docker
    }

    fn container_name(deployment: &DeploymentRecord) -> String {
        format!(
            "ignitify-svc-{}-g{}",
            deployment.service_id, deployment.generation
        )
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
}

impl ImageRuntime for DockerRuntime {
    fn runtime_ref(&self, deployment: &DeploymentRecord) -> String {
        Self::container_name(deployment)
    }

    async fn start(
        &self,
        deployment: &DeploymentRecord,
        environment: Vec<String>,
    ) -> std::result::Result<String, ControlError> {
        self.start_with_routes(deployment, environment, Vec::new())
            .await
    }

    async fn reconcile_routes(
        &self,
        deployment: &DeploymentRecord,
        environment: Vec<String>,
        routes: Vec<IngressRoute>,
    ) -> std::result::Result<bool, ControlError> {
        let Some(runtime_ref) = deployment.runtime_ref.as_deref() else {
            return Ok(false);
        };
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
        deployment: &DeploymentRecord,
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
    ) -> std::result::Result<Vec<NewDeploymentLog>, ControlError> {
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
                records.push(NewDeploymentLog {
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
        deployment: &DeploymentRecord,
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
                    image: Some(image_reference.clone()),
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
    #[error(transparent)]
    Docker(#[from] bollard::errors::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use std::env;

    use bollard::{
        container::{InspectContainerOptions, RemoveContainerOptions},
        models::HostConfig,
    };
    use ignitify_control_plane::ImageRuntime;
    use ignitify_domain::{DeploymentId, DeploymentState, ServiceId, ServiceSpec};

    use super::{DockerRuntime, GENERATION_LABEL, MANAGED_LABEL, SERVICE_LABEL};

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
        let deployment = ignitify_db::DeploymentRecord {
            id: DeploymentId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
            service_id: service_id.clone(),
            generation: 1,
            idempotency_key: "docker-test".to_owned(),
            requested_by_user_id: uuid::Uuid::new_v4().to_string(),
            spec: ServiceSpec::image(
                "caddy:2.11.4-alpine@sha256:98eb57d882ccd5213d1688764db10c1ca2c58a1ca3a6717a3411ad798f7a423a",
                Some(80),
                None,
            )
            .unwrap(),
            variables_ciphertext: "unused".to_owned(),
            runtime_ref: None,
            state: DeploymentState::Queued,
            failure_reason: None,
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            started_at: None,
            finished_at: None,
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
