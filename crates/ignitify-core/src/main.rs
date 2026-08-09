mod error;
mod operations;
mod runtime_secrets;
mod system_metrics;

use std::{env, net::SocketAddr, path::PathBuf, sync::Arc};

use ignitify_auth::{AuthConfig, AuthService};
use ignitify_control_plane::{
    AgeCipher, ControlHandle, RuntimeSelector, ServiceControl, SystemMetricsProvider,
    WorkerDependencies, WorkerHealth, spawn_worker_with_source_and_dns,
};
use ignitify_db::{Database, DatabaseConfig};
use ignitify_dns::SystemDnsVerifier;
use ignitify_ingress_traefik::TraefikIngress;
use ignitify_runtime_compose::ComposeRuntime;
use ignitify_runtime_docker::DockerRuntime;
use ignitify_source_git::GitSourceBuild;
use tokio::net::TcpListener;
use url::Url;

use crate::error::{CoreError, Result};

type RuntimeCapabilities = (
    Option<ServiceControl>,
    Option<ControlHandle>,
    Arc<dyn ignitify_control_plane::RuntimeHealth>,
    Arc<dyn ignitify_control_plane::RuntimeHealth>,
    Arc<dyn ignitify_control_plane::RuntimeHealth>,
    Option<DockerRuntime>,
);

fn trusted_origins(listener_address: SocketAddr, remote_mode: bool) -> Result<Arc<[String]>> {
    let origins = env_value("IGNITIFY_TRUSTED_ORIGINS")
        .map(|origins| {
            origins
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_else(|| AuthConfig::default().trusted_origins);
    let mut normalized = origins
        .into_iter()
        .map(|origin| normalized_origin(&origin))
        .collect::<Result<Vec<_>>>()?;
    if !remote_mode {
        for origin in embedded_listener_origins(listener_address) {
            if !normalized.iter().any(|trusted| trusted == &origin) {
                normalized.push(origin);
            }
        }
    }
    if normalized.is_empty() {
        return Err(CoreError::Configuration(
            "IGNITIFY_TRUSTED_ORIGINS must contain at least one origin",
        ));
    }
    Ok(normalized.into())
}

fn embedded_listener_origins(listener_address: SocketAddr) -> [String; 2] {
    let host = match listener_address.ip() {
        std::net::IpAddr::V4(_) => "127.0.0.1".to_owned(),
        std::net::IpAddr::V6(address) => format!("[{address}]"),
    };
    [
        format!("http://{host}:{}", listener_address.port()),
        format!("http://localhost:{}", listener_address.port()),
    ]
}

fn normalized_origin(value: &str) -> Result<String> {
    let parsed =
        Url::parse(value).map_err(|_| CoreError::Configuration("trusted origin is invalid"))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || !matches!(parsed.path(), "" | "/")
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(CoreError::Configuration(
            "trusted origin must be an HTTP(S) origin",
        ));
    }
    Ok(parsed.origin().ascii_serialization())
}

fn bool_env(name: &str, default: bool) -> Result<bool> {
    match env_value(name).as_deref() {
        None => Ok(default),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(_) => Err(CoreError::Configuration(
            "boolean environment values must be true or false",
        )),
    }
}

fn listen_address() -> Result<SocketAddr> {
    env_value("IGNITIFY_LISTEN_ADDR")
        .unwrap_or_else(|| "127.0.0.1:5656".to_owned())
        .parse()
        .map_err(|_| {
            CoreError::Configuration("IGNITIFY_LISTEN_ADDR must be an IP address and port")
        })
}

fn bootstrap_secret() -> Result<Option<String>> {
    let secret = env_value("IGNITIFY_BOOTSTRAP_SECRET");
    if secret
        .as_deref()
        .is_some_and(|value| !(32..=1024).contains(&value.len()))
    {
        return Err(CoreError::Configuration(
            "IGNITIFY_BOOTSTRAP_SECRET must be 32-1024 bytes",
        ));
    }
    Ok(secret)
}

fn remote_mode(address: SocketAddr) -> Result<bool> {
    if !address.ip().is_loopback() {
        return Err(CoreError::Configuration(
            "IGNITIFY_LISTEN_ADDR must remain loopback; place remote access behind a TLS reverse proxy",
        ));
    }
    bool_env("IGNITIFY_REMOTE_MODE", false)
}

fn validate_remote_configuration(
    remote_mode: bool,
    secure_cookies: bool,
    trusted_origins: &[String],
) -> Result<bool> {
    if !remote_mode {
        return Ok(false);
    }
    if !secure_cookies {
        return Err(CoreError::Configuration(
            "remote mode requires IGNITIFY_SECURE_COOKIES=true",
        ));
    }
    if trusted_origins
        .iter()
        .any(|origin| !origin.starts_with("https://"))
    {
        return Err(CoreError::Configuration(
            "remote mode requires HTTPS IGNITIFY_TRUSTED_ORIGINS",
        ));
    }
    Ok(true)
}

fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let listener_address = listen_address()?;
    let secure_cookies = bool_env("IGNITIFY_SECURE_COOKIES", true)?;
    let remote_mode = remote_mode(listener_address)?;
    let trusted_origins = trusted_origins(listener_address, remote_mode)?;
    let remote_mode =
        validate_remote_configuration(remote_mode, secure_cookies, trusted_origins.as_ref())?;
    let trust_proxy_headers = bool_env("IGNITIFY_TRUST_PROXY_HEADERS", remote_mode)?;
    let host_terminal_enabled = bool_env("IGNITIFY_ENABLE_HOST_TERMINAL", false)?;
    let bootstrap_secret = bootstrap_secret()?;
    let data_dir = env_value("IGNITIFY_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data"));
    let database_config = DatabaseConfig {
        url: env_value("IGNITIFY_DATABASE_URL").unwrap_or_else(|| DatabaseConfig::default().url),
    };
    let runtime_secrets = runtime_secrets::RuntimeSecrets::load_or_create(
        &data_dir,
        env_value("IGNITIFY_JWT_SECRET").as_deref(),
        env_value("IGNITIFY_SECRETS_AGE_IDENTITY").as_deref(),
    )?;
    if let Some(command) = operations::Command::from_environment()? {
        operations::execute(
            command,
            &data_dir,
            &database_config,
            &runtime_secrets.secrets_age_identity,
        )
        .await?;
        return Ok(());
    }
    let database = Database::connect(&database_config).await?;
    database.ping().await?;
    let _uptime_monitor_worker = ignitify_monitoring::MonitorWorker::new(database.clone()).spawn();

    let auth = AuthService::new(
        database.clone(),
        AuthConfig {
            jwt_secret: runtime_secrets.jwt_secret.clone(),
            secure_cookies,
            trusted_origins: trusted_origins.iter().cloned().collect(),
            bootstrap_secret,
            ..AuthConfig::default()
        },
    )
    .shared();
    let secrets_identity = runtime_secrets.secrets_age_identity;
    let provider_cipher = Arc::new(AgeCipher::from_identity(&secrets_identity)?);
    let services =
        ServiceControl::new(database.services(), database.projects(), &secrets_identity)?;
    let (control, wake) = ControlHandle::new(database.deployments(), &secrets_identity)?;
    let image_runtime = DockerRuntime::from_environment().map_err(|_| CoreError::DockerRuntime)?;
    let compose_runtime = ComposeRuntime::from_environment()?;
    let metrics_runtime = image_runtime.clone();
    let runtime = RuntimeSelector::new(image_runtime.clone(), compose_runtime);
    let runtime_health: Arc<dyn ignitify_control_plane::RuntimeHealth> = Arc::new(runtime.clone());
    let ingress = TraefikIngress::with_server_settings(
        image_runtime.clone(),
        database.clone(),
        provider_cipher.clone(),
    );
    let _ingress_ready = ingress.ensure_started().await;
    let source_build = GitSourceBuild::from_environment(database.clone(), &secrets_identity)?;
    let ingress_health: Arc<dyn ignitify_control_plane::RuntimeHealth> = Arc::new(ingress.clone());
    let (_worker, worker_ready) = spawn_worker_with_source_and_dns(
        database.deployments(),
        database.domains(),
        control.worker_cipher(),
        WorkerDependencies::new(runtime, ingress, source_build)
            .with_dns_verifier(SystemDnsVerifier::new()),
        control.worker_publisher(),
        wake,
    );
    let worker_health: Arc<dyn ignitify_control_plane::RuntimeHealth> =
        Arc::new(WorkerHealth(worker_ready));
    let (services, control, runtime_health, worker_health, ingress_health, docker_runtime): RuntimeCapabilities =
        (
            Some(services),
            Some(control),
            runtime_health,
            worker_health,
            ingress_health,
            Some(metrics_runtime),
        );
    let system_metrics: Arc<dyn SystemMetricsProvider> = Arc::new(
        system_metrics::SystemMetricsCollector::new(docker_runtime.clone()),
    );
    let domain_policy = ignitify_api::DomainPolicy::from_suffixes(
        env_value("IGNITIFY_ALLOWED_DOMAIN_SUFFIXES")
            .into_iter()
            .flat_map(|suffixes| suffixes.split(',').map(str::to_owned).collect::<Vec<_>>()),
    );
    let app = ignitify_api::router_with_system_metrics_and_docker_and_provider_cipher_and_ingress_and_domain_policy(
        auth,
        database,
        services,
        control,
        runtime_health,
        worker_health,
        system_metrics,
        docker_runtime,
        ignitify_terminal::TerminalService,
        host_terminal_enabled,
        remote_mode,
        trust_proxy_headers,
        secure_cookies,
        trusted_origins,
        Some(provider_cipher),
        ingress_health,
        domain_policy,
    );
    let listener = TcpListener::bind(listener_address).await?;

    println!(
        "Ignitify API listening on http://{}",
        listener.local_addr()?
    );
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .map_err(CoreError::Io)
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::embedded_listener_origins;

    #[test]
    fn embedded_loopback_origins_match_listener_port() {
        let address: SocketAddr = "127.0.0.1:5656".parse().unwrap();

        assert_eq!(
            embedded_listener_origins(address),
            [
                "http://127.0.0.1:5656".to_owned(),
                "http://localhost:5656".to_owned(),
            ]
        );
    }
}
