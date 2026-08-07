use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
    process::Stdio,
};

use ignitify_control_plane::{Ingress, IngressRoute, Result as ControlResult, RuntimeHealth};
use ignitify_domain::{DomainId, DomainName, ServiceId};
use ignitify_runtime_docker::DockerRuntime;
use thiserror::Error;
use tokio::process::Command;

pub const PROXY_NETWORK: &str = "ignitify-proxy";
pub const ENTRYPOINT: &str = "websecure";
pub const CERT_RESOLVER: &str = "le";
pub const INGRESS_LABEL: &str = "com.ignitify.ingress=traefik";

#[derive(Clone)]
pub struct TraefikIngress {
    runtime: DockerRuntime,
    operator: OperatorConfig,
}

impl TraefikIngress {
    pub fn new(runtime: DockerRuntime) -> Self {
        Self {
            runtime,
            operator: OperatorConfig::from_environment(),
        }
    }

    pub async fn ready(&self) -> bool {
        let network = self.runtime.network_exists(PROXY_NETWORK).await;
        let ingress = self
            .runtime
            .has_running_container_with_label(INGRESS_LABEL)
            .await;
        matches!(network, Ok(true)) && matches!(ingress, Ok(true))
    }

    pub async fn ensure_started(&self) -> bool {
        if self.ready().await {
            return true;
        }
        if !self.operator.auto_start {
            return false;
        }
        if let Err(error) = self.operator.start().await {
            tracing::warn!(error = %error, "could not start the Traefik operator stack");
            return false;
        }
        self.ready().await
    }
}

#[derive(Clone)]
struct OperatorConfig {
    auto_start: bool,
    docker_bin: String,
    compose_file: PathBuf,
}

impl OperatorConfig {
    fn from_environment() -> Self {
        Self {
            auto_start: env::var("IGNITIFY_AUTO_START_INGRESS")
                .map(|value| value.trim() != "false")
                .unwrap_or(true),
            docker_bin: env::var("IGNITIFY_DOCKER_BIN")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "docker".to_owned()),
            compose_file: env::var("IGNITIFY_TRAEFIK_COMPOSE_FILE")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("infra/traefik/compose.yaml")),
        }
    }

    async fn start(&self) -> std::result::Result<(), OperatorError> {
        let compose_file = self
            .compose_file
            .file_name()
            .ok_or(OperatorError::InvalidComposePath)?;
        let working_dir = self
            .compose_file
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        if !self.compose_file.is_file() {
            return Err(OperatorError::ComposeFileMissing(self.compose_file.clone()));
        }
        let status = Command::new(&self.docker_bin)
            .args(["compose", "-f"])
            .arg(compose_file)
            .args(["up", "-d"])
            .current_dir(working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map_err(OperatorError::Command)?;
        if status.success() {
            Ok(())
        } else {
            Err(OperatorError::CommandFailed)
        }
    }
}

#[derive(Debug, Error)]
enum OperatorError {
    #[error("Traefik compose path is invalid")]
    InvalidComposePath,
    #[error("Traefik compose file is missing: {0}")]
    ComposeFileMissing(PathBuf),
    #[error("could not execute Docker Compose")]
    Command(#[source] std::io::Error),
    #[error("Docker Compose returned a failure status")]
    CommandFailed,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("internal route port must be between 1 and 65535")]
    InvalidPort,
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn render_route(
    domain_id: &DomainId,
    hostname: &DomainName,
    port: u32,
) -> Result<IngressRoute> {
    if !(1..=65_535).contains(&port) {
        return Err(Error::InvalidPort);
    }
    let name = format!("ignitify-{}", domain_id);
    let router = format!("traefik.http.routers.{name}");
    let service = format!("traefik.http.services.{name}");
    let labels = BTreeMap::from([
        ("traefik.enable".to_owned(), "true".to_owned()),
        (format!("{router}.rule"), format!("Host(`{hostname}`)")),
        (format!("{router}.entrypoints"), ENTRYPOINT.to_owned()),
        (format!("{router}.tls"), "true".to_owned()),
        (
            format!("{router}.tls.certresolver"),
            CERT_RESOLVER.to_owned(),
        ),
        (
            format!("{service}.loadbalancer.server.port"),
            port.to_string(),
        ),
    ]);
    Ok(IngressRoute {
        labels,
        network: PROXY_NETWORK.to_owned(),
    })
}

impl Ingress for TraefikIngress {
    fn route(
        &self,
        _service_id: &ServiceId,
        domain_id: &DomainId,
        hostname: &DomainName,
        port: u32,
    ) -> ControlResult<IngressRoute> {
        render_route(domain_id, hostname, port).map_err(|_| ignitify_control_plane::Error::Runtime)
    }
}

impl RuntimeHealth for TraefikIngress {
    fn ready(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
        Box::pin(async move { self.ready().await })
    }
}

#[cfg(test)]
mod tests {
    use ignitify_domain::{DomainId, DomainName};

    use super::{CERT_RESOLVER, ENTRYPOINT, PROXY_NETWORK, render_route};

    #[test]
    fn route_labels_are_platform_owned_and_fixed() {
        let domain_id = DomainId::new("00000000-0000-4000-8000-000000000001").unwrap();
        let hostname = DomainName::new("app.example.com").unwrap();
        let route = render_route(&domain_id, &hostname, 8080).unwrap();
        assert_eq!(route.network, PROXY_NETWORK);
        assert_eq!(
            route.labels["traefik.http.routers.ignitify-00000000-0000-4000-8000-000000000001.entrypoints"],
            ENTRYPOINT
        );
        assert_eq!(
            route.labels["traefik.http.routers.ignitify-00000000-0000-4000-8000-000000000001.tls.certresolver"],
            CERT_RESOLVER
        );
        assert_eq!(route.labels.len(), 6);
        assert!(route.labels.keys().all(|key| key.starts_with("traefik.")));
    }

    #[test]
    fn route_rejects_invalid_port() {
        let domain_id = DomainId::new("00000000-0000-4000-8000-000000000001").unwrap();
        let hostname = DomainName::new("app.example.com").unwrap();
        assert!(render_route(&domain_id, &hostname, 0).is_err());
    }
}
