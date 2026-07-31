use axum::{
    Router,
    routing::{get, post},
};

use crate::{handlers, state::AppState};

pub(crate) fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health::health))
        .route(
            "/api/v1/auth/bootstrap",
            get(handlers::auth::bootstrap_status).post(handlers::auth::bootstrap),
        )
        .route("/api/v1/auth/login", post(handlers::auth::login))
        .route("/api/v1/auth/refresh", post(handlers::auth::refresh))
        .route("/api/v1/auth/logout", post(handlers::auth::logout))
        .route("/api/v1/auth/me", get(handlers::auth::me))
        .route(
            "/api/v1/projects",
            get(handlers::projects::list).post(handlers::projects::create),
        )
        .route(
            "/api/v1/projects/{project_id}",
            get(handlers::projects::get).patch(handlers::projects::update),
        )
        .with_state(state)
}
