//! Axum API routes and HTTP adapters for Ignitify.

mod error;
mod extract;
mod handlers;
mod routes;
mod state;

use std::sync::Arc;

use axum::Router;
use ignitify_auth::AuthService;
use ignitify_db::Database;

use crate::state::AppState;

/// Builds all Ignitify HTTP routes from initialized runtime dependencies.
pub fn router(
    auth: Arc<AuthService>,
    database: Database,
    secure_cookies: bool,
    trusted_origins: Arc<[String]>,
) -> Router {
    routes::router(AppState {
        auth,
        database,
        secure_cookies,
        trusted_origins,
    })
}

#[cfg(test)]
mod tests;
