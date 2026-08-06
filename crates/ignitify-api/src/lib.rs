//! Axum API routes and HTTP adapters for Ignitify.

mod error;
mod extract;
mod handlers;
mod routes;
mod state;

use std::sync::Arc;

use axum::Router;
use ignitify_auth::AuthService;
use ignitify_control_plane::{
    ControlHandle, RuntimeHealth, ServiceControl, StaticSystemMetrics, SystemMetricsProvider,
};
use ignitify_db::Database;
use ignitify_runtime_docker::DockerRuntime;

use crate::state::AppState;

/// Builds all Ignitify HTTP routes from initialized runtime dependencies.
#[expect(
    clippy::too_many_arguments,
    reason = "router composes independent runtime dependencies"
)]
pub fn router(
    auth: Arc<AuthService>,
    database: Database,
    services: Option<ServiceControl>,
    control: Option<ControlHandle>,
    runtime_health: Arc<dyn RuntimeHealth>,
    worker_health: Arc<dyn RuntimeHealth>,
    secure_cookies: bool,
    trusted_origins: Arc<[String]>,
) -> Router {
    router_with_system_metrics(
        auth,
        database,
        services,
        control,
        runtime_health,
        worker_health,
        Arc::new(StaticSystemMetrics(None)),
        ignitify_terminal::TerminalService,
        secure_cookies,
        trusted_origins,
    )
}

/// Builds all Ignitify HTTP routes with a live system metrics provider.
#[expect(
    clippy::too_many_arguments,
    reason = "router composes independent runtime dependencies"
)]
pub fn router_with_system_metrics(
    auth: Arc<AuthService>,
    database: Database,
    services: Option<ServiceControl>,
    control: Option<ControlHandle>,
    runtime_health: Arc<dyn RuntimeHealth>,
    worker_health: Arc<dyn RuntimeHealth>,
    system_metrics: Arc<dyn SystemMetricsProvider>,
    terminal: ignitify_terminal::TerminalService,
    secure_cookies: bool,
    trusted_origins: Arc<[String]>,
) -> Router {
    router_with_system_metrics_and_docker(
        auth,
        database,
        services,
        control,
        runtime_health,
        worker_health,
        system_metrics,
        None,
        terminal,
        secure_cookies,
        trusted_origins,
    )
}

/// Builds all Ignitify HTTP routes with live system metrics and Docker controls.
#[expect(
    clippy::too_many_arguments,
    reason = "router composes independent runtime dependencies"
)]
pub fn router_with_system_metrics_and_docker(
    auth: Arc<AuthService>,
    database: Database,
    services: Option<ServiceControl>,
    control: Option<ControlHandle>,
    runtime_health: Arc<dyn RuntimeHealth>,
    worker_health: Arc<dyn RuntimeHealth>,
    system_metrics: Arc<dyn SystemMetricsProvider>,
    docker_runtime: Option<DockerRuntime>,
    terminal: ignitify_terminal::TerminalService,
    secure_cookies: bool,
    trusted_origins: Arc<[String]>,
) -> Router {
    routes::router(AppState {
        auth,
        database,
        services,
        control,
        runtime_health,
        worker_health,
        system_metrics,
        docker_runtime,
        terminal,
        secure_cookies,
        trusted_origins,
    })
}

#[cfg(test)]
mod tests;
