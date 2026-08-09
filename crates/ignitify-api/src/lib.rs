//! Axum API routes and HTTP adapters for Ignitify.

mod audit;
mod domain_policy;
mod error;
mod extract;
mod frontend;
mod handlers;
mod routes;
mod state;

use std::{collections::HashMap, sync::Arc};

use axum::Router;
use ignitify_auth::AuthService;
use ignitify_control_plane::{
    AgeCipher, ControlHandle, RuntimeHealth, ServiceControl, StaticSystemMetrics,
    SystemMetricsProvider,
};
use ignitify_db::Database;
use ignitify_runtime_docker::DockerRuntime;

use crate::state::AppState;
pub use domain_policy::DomainPolicy;

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
    router_with_system_metrics_and_docker_and_provider_cipher(
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
        None,
    )
}

/// Builds all Ignitify HTTP routes with provider credential encryption enabled.
#[expect(
    clippy::too_many_arguments,
    reason = "router composes independent runtime dependencies"
)]
pub fn router_with_system_metrics_and_docker_and_provider_cipher(
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
    provider_cipher: Option<Arc<AgeCipher>>,
) -> Router {
    router_with_system_metrics_and_docker_and_provider_cipher_and_ingress(
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
        provider_cipher,
        Arc::new(ignitify_control_plane::StaticRuntimeHealth(false)),
    )
}

/// Builds all Ignitify HTTP routes with live ingress readiness reporting.
#[expect(
    clippy::too_many_arguments,
    reason = "router composes independent runtime dependencies"
)]
pub fn router_with_system_metrics_and_docker_and_provider_cipher_and_ingress(
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
    provider_cipher: Option<Arc<AgeCipher>>,
    ingress_health: Arc<dyn RuntimeHealth>,
) -> Router {
    router_with_system_metrics_and_docker_and_provider_cipher_and_ingress_and_domain_policy(
        auth,
        database,
        services,
        control,
        runtime_health,
        worker_health,
        system_metrics,
        docker_runtime,
        terminal,
        false,
        false,
        false,
        secure_cookies,
        trusted_origins,
        provider_cipher,
        ingress_health,
        DomainPolicy::permissive(),
    )
}

/// Builds all Ignitify HTTP routes with live ingress readiness and a public-domain policy.
#[expect(
    clippy::too_many_arguments,
    reason = "router composes independent runtime dependencies"
)]
pub fn router_with_system_metrics_and_docker_and_provider_cipher_and_ingress_and_domain_policy(
    auth: Arc<AuthService>,
    database: Database,
    services: Option<ServiceControl>,
    control: Option<ControlHandle>,
    runtime_health: Arc<dyn RuntimeHealth>,
    worker_health: Arc<dyn RuntimeHealth>,
    system_metrics: Arc<dyn SystemMetricsProvider>,
    docker_runtime: Option<DockerRuntime>,
    terminal: ignitify_terminal::TerminalService,
    host_terminal_enabled: bool,
    require_explicit_origin: bool,
    trust_proxy_headers: bool,
    secure_cookies: bool,
    trusted_origins: Arc<[String]>,
    provider_cipher: Option<Arc<AgeCipher>>,
    ingress_health: Arc<dyn RuntimeHealth>,
    domain_policy: DomainPolicy,
) -> Router {
    routes::router(AppState {
        auth,
        database,
        services,
        control,
        runtime_health,
        worker_health,
        ingress_health,
        system_metrics,
        docker_runtime,
        terminal,
        host_terminal_enabled,
        terminal_sessions: Arc::new(tokio::sync::Semaphore::new(4)),
        login_rate_limiter: state::LoginRateLimiter::default(),
        require_explicit_origin,
        trust_proxy_headers,
        secure_cookies,
        trusted_origins,
        provider_cipher,
        domain_policy,
        github_manifest_states: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
    })
}

#[cfg(test)]
mod tests;
