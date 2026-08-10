use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::HeaderValue,
    middleware,
    response::Response,
    routing::{get, post},
};

use crate::{frontend, handlers, openapi, state::AppState};
use utoipa_swagger_ui::SwaggerUi;

pub(crate) fn router(state: AppState) -> Router {
    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", openapi::document()))
        .route("/health", get(handlers::health::health))
        .route(
            "/api/v1/webhooks/services/{service_id}",
            post(handlers::webhooks::receive_push).layer(DefaultBodyLimit::max(
                handlers::webhooks::WEBHOOK_BODY_LIMIT,
            )),
        )
        .route(
            "/api/v1/auth/bootstrap",
            get(handlers::auth::bootstrap_status).post(handlers::auth::bootstrap),
        )
        .route("/api/v1/auth/login", post(handlers::auth::login))
        .route("/api/v1/auth/refresh", post(handlers::auth::refresh))
        .route("/api/v1/auth/logout", post(handlers::auth::logout))
        .route("/api/v1/auth/step-up", post(handlers::auth::step_up))
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
            "/api/v1/settings/infrastructure",
            get(handlers::settings::get).patch(handlers::settings::update),
        )
        .route(
            "/api/v1/settings/infrastructure/certificates",
            axum::routing::post(handlers::settings::create_certificate),
        )
        .route(
            "/api/v1/settings/infrastructure/certificates/{certificate_id}",
            axum::routing::delete(handlers::settings::remove_certificate),
        )
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
            "/api/v1/settings/backup-destination/s3",
            get(handlers::backup_destinations::get)
                .put(handlers::backup_destinations::upsert)
                .patch(handlers::backup_destinations::update_controls)
                .delete(handlers::backup_destinations::remove),
        )
        .route(
            "/api/v1/settings/backup-destination/s3/runs",
            get(handlers::backup_destinations::list_runs),
        )
        .route(
            "/api/v1/remote-builders",
            get(handlers::remote_builders::list).post(handlers::remote_builders::create),
        )
        .route(
            "/api/v1/remote-builders/{builder_id}",
            axum::routing::delete(handlers::remote_builders::remove)
                .patch(handlers::remote_builders::update),
        )
        .route(
            "/api/v1/remote-builders/{builder_id}/default",
            post(handlers::remote_builders::make_default),
        )
        .route(
            "/api/v1/remote-servers",
            get(handlers::remote_servers::list).post(handlers::remote_servers::create),
        )
        .route(
            "/api/v1/remote-servers/{server_id}",
            axum::routing::delete(handlers::remote_servers::remove)
                .patch(handlers::remote_servers::update),
        )
        .route(
            "/api/v1/remote-servers/{server_id}/default",
            post(handlers::remote_servers::make_default),
        )
          .route(
              "/api/v1/remote-servers/{server_id}/check",
              post(handlers::remote_servers::check),
          )
          .route(
              "/api/v1/remote-servers/{server_id}/access",
              get(handlers::remote_servers::access),
          )
          .route(
            "/api/v1/remote-servers/{server_id}/agent",
            get(handlers::remote_agent::status),
        )
        .route(
            "/api/v1/remote-servers/{server_id}/agent/install",
            post(handlers::remote_agent::install),
        )
        .route(
            "/api/v1/remote-agents/heartbeat",
            post(handlers::remote_agent::heartbeat),
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
        .route(
            "/api/v1/uptime-monitors",
            get(handlers::uptime_monitors::list).post(handlers::uptime_monitors::create),
        )
        .route(
            "/api/v1/uptime-monitors/{monitor_id}",
            axum::routing::delete(handlers::uptime_monitors::remove)
                .patch(handlers::uptime_monitors::update),
        )
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
            "/api/v1/services/{service_id}/auto-deploy-secret",
            post(handlers::services::rotate_auto_deploy_secret),
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
            "/api/v1/deployments/{deployment_id}/cancel",
            post(handlers::deployments::cancel),
        )
        .route(
            "/api/v1/domains/{domain_id}",
            axum::routing::delete(handlers::domains::remove),
        )
        .route(
            "/api/v1/domains/{domain_id}/verify",
            post(handlers::domains::verify),
        )
        .fallback(frontend::serve)
        .layer(middleware::map_response(security_headers))
        .with_state(state)
}

async fn security_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    if !headers.contains_key("cache-control") {
        headers.insert("cache-control", HeaderValue::from_static("no-store"));
    }
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; connect-src 'self' wss:; base-uri 'self'; frame-ancestors 'none'; form-action 'self'; object-src 'none'",
        ),
    );
    headers.insert(
        "cross-origin-opener-policy",
        HeaderValue::from_static("same-origin"),
    );
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    response
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{HeaderValue, header},
        response::Response,
    };

    use super::security_headers;

    #[tokio::test]
    async fn security_headers_preserve_frontend_asset_cache_policy() {
        let mut response = Response::new(Body::empty());
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );

        let response = security_headers(response).await;

        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static(
                "public, max-age=31536000, immutable"
            ))
        );
        assert_eq!(
            response.headers().get("x-content-type-options"),
            Some(&HeaderValue::from_static("nosniff"))
        );
    }
}
