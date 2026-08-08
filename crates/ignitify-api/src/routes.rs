use axum::{
    Router,
    extract::DefaultBodyLimit,
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
        .route(
            "/api/v1/providers",
            get(handlers::providers::list).post(handlers::providers::create),
        )
        .route(
            "/api/v1/providers/github/manifest",
            axum::routing::post(handlers::providers::start_github_manifest),
        )
        .route(
            "/api/v1/providers/github/manifest/callback",
            get(handlers::providers::github_manifest_callback),
        )
        .route(
            "/api/v1/providers/{provider_id}",
            axum::routing::delete(handlers::providers::remove).patch(handlers::providers::update),
        )
        .route(
            "/api/v1/providers/{provider_id}/test",
            axum::routing::post(handlers::provider_test::test),
        )
        .route(
            "/api/v1/providers/{provider_id}/repositories",
            get(handlers::provider_test::repositories),
        )
        .route(
            "/api/v1/providers/{provider_id}/branches",
            get(handlers::provider_test::branches),
        )
        .route("/api/v1/runtime/status", get(handlers::runtime::status))
        .route(
            "/api/v1/settings/server",
            get(handlers::settings::get).patch(handlers::settings::update),
        )
        .route(
            "/api/v1/settings/server/certificates",
            axum::routing::post(handlers::settings::create_certificate),
        )
        .route(
            "/api/v1/settings/server/certificates/{certificate_id}",
            axum::routing::delete(handlers::settings::remove_certificate),
        )
        .route(
            "/api/v1/runtime/containers",
            get(handlers::runtime::containers),
        )
        .route(
            "/api/v1/runtime/containers/{container_id}/details",
            get(handlers::runtime::container_details),
        )
        .route(
            "/api/v1/runtime/containers/{container_id}/logs",
            get(handlers::runtime::container_logs),
        )
        .route(
            "/api/v1/runtime/containers/{container_id}/upload",
            post(handlers::runtime::upload_container_file)
                .layer(DefaultBodyLimit::max(8 * 1024 * 1024)),
        )
        .route(
            "/api/v1/runtime/containers/{container_id}/terminal",
            get(handlers::terminal::container),
        )
        .route(
            "/api/v1/runtime/containers/{container_id}",
            axum::routing::delete(handlers::runtime::remove_container),
        )
        .route("/api/v1/runtime/metrics", get(handlers::runtime::metrics))
        .route("/api/v1/terminal", get(handlers::terminal::open))
        .route(
            "/api/v1/projects",
            get(handlers::projects::list).post(handlers::projects::create),
        )
        .route(
            "/api/v1/projects/{project_id}",
            get(handlers::projects::get)
                .patch(handlers::projects::update)
                .delete(handlers::projects::remove),
        )
        .route(
            "/api/v1/projects/{project_id}/environment",
            get(handlers::project_environment::get).put(handlers::project_environment::update),
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
            "/api/v1/projects/{project_id}/services",
            get(handlers::services::list).post(handlers::services::create),
        )
        .route(
            "/api/v1/services/{service_id}",
            get(handlers::services::get)
                .patch(handlers::services::update)
                .delete(handlers::services::remove),
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
        .with_state(state)
}
