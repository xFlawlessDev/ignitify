mod error;
mod runtime_secrets;
mod system_metrics;

use std::{env, path::PathBuf, sync::Arc};

use ignitify_auth::{AuthConfig, AuthService};
use ignitify_control_plane::{
    AgeCipher, ControlHandle, RuntimeSelector, ServiceControl, SystemMetricsProvider,
    WorkerDependencies, WorkerHealth, spawn_worker_with_source,
};
use ignitify_db::{Database, DatabaseConfig};
use ignitify_ingress_traefik::TraefikIngress;
use ignitify_runtime_compose::ComposeRuntime;
use ignitify_runtime_docker::DockerRuntime;
use ignitify_source_git::GitSourceBuild;
use tokio::net::TcpListener;

use crate::error::{CoreError, Result};

type RuntimeCapabilities = (
    Option<ServiceControl>,
    Option<ControlHandle>,
    Arc<dyn ignitify_control_plane::RuntimeHealth>,
    Arc<dyn ignitify_control_plane::RuntimeHealth>,
    Arc<dyn ignitify_control_plane::RuntimeHealth>,
    Option<DockerRuntime>,
);

fn trusted_origins() -> Arc<[String]> {
    env_value("IGNITIFY_TRUSTED_ORIGINS")
        .map(|origins| {
            origins
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_else(|| AuthConfig::default().trusted_origins)
        .into()
}

fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let data_dir = env_value("IGNITIFY_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data"));
    let runtime_secrets = runtime_secrets::RuntimeSecrets::load_or_create(
        &data_dir,
        env_value("IGNITIFY_JWT_SECRET").as_deref(),
        env_value("IGNITIFY_SECRETS_AGE_IDENTITY").as_deref(),
    )?;
    let database = Database::connect(&DatabaseConfig {
        url: env_value("IGNITIFY_DATABASE_URL").unwrap_or_else(|| DatabaseConfig::default().url),
    })
    .await?;
    database.ping().await?;

    let auth = AuthService::new(
        database.clone(),
        AuthConfig {
            jwt_secret: runtime_secrets.jwt_secret.clone(),
            ..AuthConfig::default()
        },
    )
    .shared();
    let secrets_identity = runtime_secrets.secrets_age_identity;
    let provider_cipher = Some(Arc::new(AgeCipher::from_identity(&secrets_identity)?));
    let services =
        ServiceControl::new(database.services(), database.projects(), &secrets_identity)?;
    let (control, wake) = ControlHandle::new(database.deployments(), &secrets_identity)?;
    let image_runtime = DockerRuntime::from_environment().map_err(|_| CoreError::DockerRuntime)?;
    let compose_runtime = ComposeRuntime::from_environment()?;
    let metrics_runtime = image_runtime.clone();
    let runtime = RuntimeSelector::new(image_runtime.clone(), compose_runtime);
    let runtime_health: Arc<dyn ignitify_control_plane::RuntimeHealth> = Arc::new(runtime.clone());
    let ingress = TraefikIngress::new(image_runtime.clone());
    let _ingress_ready = ingress.ensure_started().await;
    let source_build = GitSourceBuild::from_environment(database.clone(), &secrets_identity)?;
    let ingress_health: Arc<dyn ignitify_control_plane::RuntimeHealth> = Arc::new(ingress.clone());
    let (_worker, worker_ready) = spawn_worker_with_source(
        database.deployments(),
        database.domains(),
        control.worker_cipher(),
        WorkerDependencies::new(runtime, ingress, source_build),
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
    let app = ignitify_api::router_with_system_metrics_and_docker_and_provider_cipher_and_ingress(
        auth,
        database,
        services,
        control,
        runtime_health,
        worker_health,
        system_metrics,
        docker_runtime,
        ignitify_terminal::TerminalService,
        env_value("IGNITIFY_SECURE_COOKIES").is_some_and(|value| value == "true"),
        trusted_origins(),
        provider_cipher,
        ingress_health,
    );
    let listener = TcpListener::bind("127.0.0.1:5656").await?;

    println!("Ignitify API listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await.map_err(CoreError::Io)
}
