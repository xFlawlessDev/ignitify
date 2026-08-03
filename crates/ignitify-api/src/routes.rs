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
        .route("/api/v1/dashboard", get(handlers::dashboard::get))
        .route("/api/v1/runtime/status", get(handlers::runtime::status))
        .route(
            "/api/v1/services/{service_id}/terminal/capability",
            get(handlers::terminal::capability),
        )
        .route(
            "/api/v1/registries",
            get(handlers::registries::list).post(handlers::registries::create),
        )
        .route(
            "/api/v1/registries/{registry_id}",
            axum::routing::delete(handlers::registries::remove),
        )
        .route(
            "/api/v1/projects",
            get(handlers::projects::list).post(handlers::projects::create),
        )
        .route(
            "/api/v1/projects/{project_id}",
            get(handlers::projects::get).patch(handlers::projects::update),
        )
        .route(
            "/api/v1/projects/{project_id}/deployments",
            get(handlers::deployments::list_for_project),
        )
        .route(
            "/api/v1/projects/{project_id}/activity",
            get(handlers::activity::list_for_project),
        )
        .route(
            "/api/v1/projects/{project_id}/webhooks",
            get(handlers::webhooks::list).post(handlers::webhooks::create),
        )
        .route(
            "/api/v1/projects/{project_id}/services",
            get(handlers::services::list).post(handlers::services::create),
        )
        .route(
            "/api/v1/services/{service_id}",
            get(handlers::services::get).patch(handlers::services::update),
        )
        .route(
            "/api/v1/services/{service_id}/deployments",
            get(handlers::deployments::list).post(handlers::deployments::deploy),
        )
        .route(
            "/api/v1/services/{service_id}/domains",
            get(handlers::domains::list).post(handlers::domains::create),
        )
        .route(
            "/api/v1/services/{service_id}/stop",
            post(handlers::deployments::stop),
        )
        .route(
            "/api/v1/deployments/{deployment_id}",
            get(handlers::deployments::get),
        )
        .route(
            "/api/v1/deployments/{deployment_id}/events",
            get(handlers::streams::events),
        )
        .route(
            "/api/v1/deployments/{deployment_id}/logs",
            get(handlers::streams::logs),
        )
        .route(
            "/api/v1/deployments/{deployment_id}/rollback",
            post(handlers::deployments::rollback),
        )
        .route(
            "/api/v1/domains/{domain_id}",
            axum::routing::delete(handlers::domains::remove),
        )
        .route(
            "/api/v1/webhooks/{webhook_id}",
            axum::routing::delete(handlers::webhooks::remove),
        )
        .with_state(state)
}
