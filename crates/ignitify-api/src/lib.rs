//! Axum API routes and HTTP adapters for Ignitify.

mod error;
mod extract;
mod handlers;
mod routes;
mod state;

use std::sync::Arc;

use axum::Router;
use ignitify_auth::AuthService;
use ignitify_control_plane::{ControlHandle, RuntimeHealth, ServiceControl};
use ignitify_db::Database;

use crate::state::AppState;

/// Builds all Ignitify HTTP routes from initialized runtime dependencies.
#[expect(
    clippy::too_many_arguments,
    reason = "router composes independent runtime dependencies"
)]
pub fn router(
    auth: Arc<AuthService>,
    database: Database,
    services: ServiceControl,
    control: ControlHandle,
    runtime_health: Arc<dyn RuntimeHealth>,
    worker_health: Arc<dyn RuntimeHealth>,
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
        secure_cookies,
        trusted_origins,
    })
}

#[cfg(test)]
mod tests;
