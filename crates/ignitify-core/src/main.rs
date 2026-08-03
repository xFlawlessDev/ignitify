mod error;

use std::{env, sync::Arc};

use ignitify_auth::{AuthConfig, AuthService};
use ignitify_control_plane::{
    ControlHandle, RuntimeSelector, ServiceControl, WorkerHealth, spawn_worker,
};
use ignitify_db::{Database, DatabaseConfig};
use ignitify_ingress_traefik::TraefikIngress;
use ignitify_runtime_compose::ComposeRuntime;
use ignitify_runtime_docker::DockerRuntime;
use tokio::net::TcpListener;

use crate::error::{CoreError, Result};

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
    env::var(name).ok().or_else(|| match name {
        "IGNITIFY_DATABASE_URL" => option_env!("IGNITIFY_DATABASE_URL").map(str::to_owned),
        "IGNITIFY_JWT_SECRET" => option_env!("IGNITIFY_JWT_SECRET").map(str::to_owned),
        "IGNITIFY_SECURE_COOKIES" => option_env!("IGNITIFY_SECURE_COOKIES").map(str::to_owned),
        "IGNITIFY_TRUSTED_ORIGINS" => option_env!("IGNITIFY_TRUSTED_ORIGINS").map(str::to_owned),
        "IGNITIFY_SECRETS_AGE_IDENTITY" => {
            option_env!("IGNITIFY_SECRETS_AGE_IDENTITY").map(str::to_owned)
        }
        "IGNITIFY_DOCKER_BIN" => option_env!("IGNITIFY_DOCKER_BIN").map(str::to_owned),
        "IGNITIFY_COMPOSE_ROOT" => option_env!("IGNITIFY_COMPOSE_ROOT").map(str::to_owned),
        _ => None,
    })
}

fn required_env(name: &'static str) -> Result<String> {
    env_value(name).ok_or(CoreError::MissingEnvironment(name))
}

#[tokio::main]
async fn main() -> Result<()> {
    let database = Database::connect(&DatabaseConfig {
        url: env_value("IGNITIFY_DATABASE_URL").unwrap_or_else(|| DatabaseConfig::default().url),
    })
    .await?;
    database.ping().await?;

    let auth = AuthService::new(
        database.clone(),
        AuthConfig {
            jwt_secret: required_env("IGNITIFY_JWT_SECRET")?,
            ..AuthConfig::default()
        },
    )
    .shared();
    let secrets_identity = required_env("IGNITIFY_SECRETS_AGE_IDENTITY")?;
    let services = ServiceControl::new(database.services(), &secrets_identity)?;
    let (control, wake) = ControlHandle::new(database.deployments(), &secrets_identity)?;
    let image_runtime = DockerRuntime::from_environment().map_err(|_| CoreError::DockerRuntime)?;
    image_runtime
        .ping()
        .await
        .map_err(|_| CoreError::DockerRuntime)?;
    let compose_runtime = ComposeRuntime::from_paths(
        env_value("IGNITIFY_DOCKER_BIN").map(Into::into),
        env_value("IGNITIFY_COMPOSE_ROOT").map(Into::into),
    )?;
    let runtime = RuntimeSelector::new(image_runtime, compose_runtime);
    let runtime_health = Arc::new(runtime.clone());
    let (_worker, worker_ready) = spawn_worker(
        database.deployments(),
        database.domains(),
        control.worker_cipher(),
        runtime,
        TraefikIngress,
        control.worker_publisher(),
        wake,
    );
    let worker_health = Arc::new(WorkerHealth(worker_ready));
    let app = ignitify_api::router(
        auth,
        database,
        services,
        control,
        runtime_health,
        worker_health,
        env_value("IGNITIFY_SECURE_COOKIES").is_some_and(|value| value == "true"),
        trusted_origins(),
    );
    let listener = TcpListener::bind("127.0.0.1:5656").await?;

    println!("Ignitify API listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await.map_err(CoreError::Io)
}
