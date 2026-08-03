use std::sync::Arc;

use age::secrecy::ExposeSecret;
use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use futures_util::StreamExt;
use http_body_util::BodyExt;
use ignitify_auth::AuthConfig;
use ignitify_control_plane::{
    ControlHandle, HostRuntimeMetrics, ServiceControl, StaticRuntimeHealth,
};
use ignitify_db::{DatabaseConfig, ProjectActor, UserRole as DatabaseUserRole};
use ignitify_domain::ProjectMemberRole;
use tower::ServiceExt;

use crate::{router, state::AppState};

async fn state() -> AppState {
    let database = ignitify_db::Database::connect(&DatabaseConfig {
        url: "sqlite::memory:".to_owned(),
    })
    .await
    .unwrap();
    let auth = ignitify_auth::AuthService::new(
        database.clone(),
        AuthConfig {
            jwt_secret: "test-secret".to_owned(),
            ..AuthConfig::default()
        },
    )
    .shared();
    let identity = age::x25519::Identity::generate().to_string();
    let services = ServiceControl::new(database.services(), identity.expose_secret()).unwrap();
    let (control, _wake) =
        ControlHandle::new(database.deployments(), identity.expose_secret()).unwrap();
    AppState {
        auth,
        database,
        services,
        control,
        runtime_health: Arc::new(StaticRuntimeHealth(true)),
        worker_health: Arc::new(StaticRuntimeHealth(true)),
        secure_cookies: false,
        trusted_origins: Arc::from([]),
    }
}

#[tokio::test]
async fn health_reports_unavailable_when_docker_is_unready() {
    let mut state = state().await;
    state.runtime_health = Arc::new(StaticRuntimeHealth(false));
    let app = router(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.secure_cookies,
        state.trusted_origins.clone(),
    );

    let response = app
        .oneshot(request("GET", "/health", None, ""))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

async fn session_token(state: &AppState) -> String {
    state
        .auth
        .bootstrap_admin("owner", "password123")
        .await
        .unwrap()
        .access_token
}

fn request(method: &str, uri: &str, token: Option<&str>, body: &str) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap();
    if let Some(token) = token {
        request
            .headers_mut()
            .insert("authorization", format!("Bearer {token}").parse().unwrap());
    }
    if method != "GET" {
        request
            .headers_mut()
            .insert("x-ignitify-request", "1".parse().unwrap());
    }
    request
}

#[tokio::test]
async fn project_routes_enforce_auth_membership_and_role() {
    let state = state().await;
    let token = session_token(&state).await;
    let app = router(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.secure_cookies,
        state.trusted_origins.clone(),
    );

    let unauthenticated = app
        .clone()
        .oneshot(request("GET", "/api/v1/projects", None, ""))
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), 401);

    let unauthenticated_mutation = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Platform"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthenticated_mutation.status(), 401);

    let created = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/projects",
            Some(&token),
            r#"{"name":"Platform"}"#,
        ))
        .await
        .unwrap();
    let created_status = created.status();
    let created_body = created.into_body().collect().await.unwrap().to_bytes();
    let created_json: serde_json::Value = serde_json::from_slice(&created_body).unwrap();
    assert_eq!(
        (
            created_status,
            created_json["default_environment"]["name"].as_str()
        ),
        (StatusCode::CREATED, Some("production"))
    );

    let actor = state.auth.authenticate_bearer(&token).await.unwrap();
    let project = state
        .database
        .projects()
        .list(ProjectActor {
            id: &actor.id,
            is_admin: true,
        })
        .await
        .unwrap()
        .pop()
        .unwrap();
    let password_hash = Argon2::default()
        .hash_password(b"password123", &SaltString::generate(&mut OsRng))
        .unwrap()
        .to_string();
    let outsider = state
        .database
        .users()
        .create("outsider", &password_hash, DatabaseUserRole::User)
        .await
        .unwrap();
    let outsider_token = ignitify_auth::AuthService::new(
        state.database.clone(),
        AuthConfig {
            jwt_secret: "test-secret".to_owned(),
            ..AuthConfig::default()
        },
    )
    .login("outsider", "password123")
    .await
    .unwrap()
    .access_token;

    let hidden = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/v1/projects/{}", project.id),
            Some(&outsider_token),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(hidden.status(), 404);

    state
        .database
        .projects()
        .add_member(project.id.as_str(), &outsider.id, ProjectMemberRole::Viewer)
        .await
        .unwrap();

    let viewer = app
        .oneshot(request(
            "PATCH",
            &format!("/api/v1/projects/{}", project.id),
            Some(&outsider_token),
            r#"{"name":"Renamed"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(viewer.status(), 403);
}

#[tokio::test]
async fn deployment_route_rejects_invalid_idempotency_key() {
    let state = state().await;
    let token = session_token(&state).await;
    let app = router(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.secure_cookies,
        state.trusted_origins.clone(),
    );
    let project = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/projects",
            Some(&token),
            r#"{"name":"Platform"}"#,
        ))
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let project: serde_json::Value = serde_json::from_slice(&project).unwrap();
    let service = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/v1/projects/{}/services", project["id"].as_str().unwrap()),
            Some(&token),
            r#"{"name":"web","image_reference":"nginx@sha256:deadbeef","internal_port":null,"healthcheck":null,"variables":[]}"#,
        ))
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let service: serde_json::Value = serde_json::from_slice(&service).unwrap();
    let mut request = request(
        "POST",
        &format!(
            "/api/v1/services/{}/deployments",
            service["id"].as_str().unwrap()
        ),
        Some(&token),
        "",
    );
    request
        .headers_mut()
        .insert("idempotency-key", axum::http::HeaderValue::from_static(""));

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn deployment_events_replay_durable_rows_and_keep_unauthorized_hidden() {
    let state = state().await;
    let token = session_token(&state).await;
    let app = router(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.secure_cookies,
        state.trusted_origins.clone(),
    );
    let project = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/projects",
            Some(&token),
            r#"{"name":"Platform"}"#,
        ))
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let project: serde_json::Value = serde_json::from_slice(&project).unwrap();
    let service = app
        .clone()
        .oneshot(request(
            "POST",
            &format!(
                "/api/v1/projects/{}/services",
                project["id"].as_str().unwrap()
            ),
            Some(&token),
            r#"{"name":"web","image_reference":"nginx@sha256:deadbeef","internal_port":8080,"healthcheck":null,"variables":[]}"#,
        ))
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let service: serde_json::Value = serde_json::from_slice(&service).unwrap();
    let mut deploy_request = request(
        "POST",
        &format!(
            "/api/v1/services/{}/deployments",
            service["id"].as_str().unwrap()
        ),
        Some(&token),
        "",
    );
    deploy_request
        .headers_mut()
        .insert("idempotency-key", "stream-test".parse().unwrap());
    let deployment = app
        .clone()
        .oneshot(deploy_request)
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let deployment: serde_json::Value = serde_json::from_slice(&deployment).unwrap();
    let response = app
        .clone()
        .oneshot(request(
            "GET",
            &format!(
                "/api/v1/deployments/{}/events",
                deployment["id"].as_str().unwrap()
            ),
            Some(&token),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    let mut body = response.into_body().into_data_stream();
    let first = tokio::time::timeout(std::time::Duration::from_secs(1), body.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(String::from_utf8_lossy(&first).contains("deployment.queued"));
}

#[tokio::test]
async fn domain_routes_require_service_port_and_exact_confirmation() {
    let state = state().await;
    let token = session_token(&state).await;
    let app = router(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.secure_cookies,
        state.trusted_origins.clone(),
    );
    let project = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/projects",
            Some(&token),
            r#"{"name":"Platform"}"#,
        ))
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let project: serde_json::Value = serde_json::from_slice(&project).unwrap();
    let service = app
        .clone()
        .oneshot(request(
            "POST",
            &format!(
                "/api/v1/projects/{}/services",
                project["id"].as_str().unwrap()
            ),
            Some(&token),
            r#"{"name":"web","image_reference":"nginx@sha256:deadbeef","internal_port":8080,"healthcheck":null,"variables":[]}"#,
        ))
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let service: serde_json::Value = serde_json::from_slice(&service).unwrap();
    let created = app
        .clone()
        .oneshot(request(
            "POST",
            &format!(
                "/api/v1/services/{}/domains",
                service["id"].as_str().unwrap()
            ),
            Some(&token),
            r#"{"hostname":"app.example.com"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::ACCEPTED);
    let body = created.into_body().collect().await.unwrap().to_bytes();
    let domain: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let wrong_confirmation = app
        .clone()
        .oneshot(request(
            "DELETE",
            &format!("/api/v1/domains/{}", domain["id"].as_str().unwrap()),
            Some(&token),
            r#"{"confirm_hostname":"wrong.example.com"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(wrong_confirmation.status(), StatusCode::BAD_REQUEST);
    let removed = app
        .oneshot(request(
            "DELETE",
            &format!("/api/v1/domains/{}", domain["id"].as_str().unwrap()),
            Some(&token),
            r#"{"confirm_hostname":"app.example.com"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn service_routes_encrypt_variables_and_enforce_access() {
    let state = state().await;
    let token = session_token(&state).await;
    let app = router(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.secure_cookies,
        state.trusted_origins.clone(),
    );
    let health = app
        .clone()
        .oneshot(request("GET", "/health", None, ""))
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);
    let project = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/projects",
            Some(&token),
            r#"{"name":"Platform"}"#,
        ))
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let project: serde_json::Value = serde_json::from_slice(&project).unwrap();
    let project_id = project["id"].as_str().unwrap();
    let created = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/v1/projects/{project_id}/services"),
            Some(&token),
            r#"{"name":"web","image_reference":"nginx@sha256:deadbeef","internal_port":8080,"healthcheck":["/health"],"variables":[{"key":"TOKEN","value":"plain-secret","is_secret":true},{"key":"PORT","value":"8080","is_secret":false}]}"#,
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let body = created.into_body().collect().await.unwrap().to_bytes();
    let service: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        service["variables"]
            .as_array()
            .unwrap()
            .iter()
            .all(|variable| { variable["key"] != "TOKEN" || variable.get("value").is_none() })
    );
    assert!(!String::from_utf8_lossy(&body).contains("plain-secret"));
    let service_id = service["id"].as_str().unwrap();
    let password_hash = Argon2::default()
        .hash_password(b"password123", &SaltString::generate(&mut OsRng))
        .unwrap()
        .to_string();
    let viewer = state
        .database
        .users()
        .create("viewer", &password_hash, DatabaseUserRole::User)
        .await
        .unwrap();
    state
        .database
        .projects()
        .add_member(project_id, &viewer.id, ProjectMemberRole::Viewer)
        .await
        .unwrap();
    let viewer_token = ignitify_auth::AuthService::new(
        state.database.clone(),
        AuthConfig {
            jwt_secret: "test-secret".to_owned(),
            ..AuthConfig::default()
        },
    )
    .login("viewer", "password123")
    .await
    .unwrap()
    .access_token;
    let viewer_read = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/v1/services/{service_id}"),
            Some(&viewer_token),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(viewer_read.status(), StatusCode::OK);
    let viewer_body = viewer_read.into_body().collect().await.unwrap().to_bytes();
    let viewer_service: serde_json::Value = serde_json::from_slice(&viewer_body).unwrap();
    assert!(
        viewer_service["variables"]
            .as_array()
            .unwrap()
            .iter()
            .all(|variable| { variable.get("value").is_none() })
    );
    let viewer_update = app
        .oneshot(request(
            "PATCH",
            &format!("/api/v1/services/{service_id}"),
            Some(&viewer_token),
            r#"{"name":"web","image_reference":"nginx@sha256:deadbeef","internal_port":8080,"healthcheck":null,"variables":[]}"#,
        ))
        .await
        .unwrap();
    assert_eq!(viewer_update.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn dashboard_requires_auth_and_returns_safe_aggregate() {
    let state = state().await;
    let token = session_token(&state).await;
    let app = router(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.secure_cookies,
        state.trusted_origins.clone(),
    );

    let unauthenticated = app
        .clone()
        .oneshot(request("GET", "/api/v1/dashboard", None, ""))
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let project = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/projects",
            Some(&token),
            r#"{"name":"Platform"}"#,
        ))
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let project: serde_json::Value = serde_json::from_slice(&project).unwrap();
    let project_id = project["id"].as_str().unwrap();
    app.clone()
        .oneshot(request(
            "POST",
            &format!("/api/v1/projects/{project_id}/services"),
            Some(&token),
            r#"{"name":"web","image_reference":"nginx@sha256:deadbeef","internal_port":8080,"healthcheck":null,"variables":[{"key":"TOKEN","value":"plain-secret","is_secret":true}]}"#,
        ))
        .await
        .unwrap();

    let response = app
        .oneshot(request("GET", "/api/v1/dashboard", Some(&token), ""))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let dashboard: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(dashboard["projects"].as_array().unwrap().len(), 1);
    assert_eq!(dashboard["services"].as_array().unwrap().len(), 1);
    assert!(dashboard["services"][0].get("variables").is_none());
    assert!(!String::from_utf8_lossy(&body).contains("plain-secret"));
}

#[tokio::test]
async fn activity_registry_webhook_and_terminal_contracts_are_safe() {
    let state = state().await;
    let token = session_token(&state).await;
    let app = router(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.secure_cookies,
        state.trusted_origins.clone(),
    );
    let project = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/projects",
            Some(&token),
            r#"{"name":"Operations"}"#,
        ))
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let project: serde_json::Value = serde_json::from_slice(&project).unwrap();
    let project_id = project["id"].as_str().unwrap();

    let activity = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/v1/projects/{project_id}/activity"),
            Some(&token),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(activity.status(), StatusCode::OK);
    let activity = activity.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&activity).contains("project.create"));

    let registry = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/registries",
            Some(&token),
            r#"{"name":"private","endpoint":"https://registry.example.com","username":"deploy","credential":"secret"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(registry.status(), StatusCode::CREATED);
    let registry = registry.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&registry).contains("credential_configured"));
    assert!(!String::from_utf8_lossy(&registry).contains("secret"));
    let registry: serde_json::Value = serde_json::from_slice(&registry).unwrap();
    let wrong_registry_confirmation = app
        .clone()
        .oneshot(request(
            "DELETE",
            &format!("/api/v1/registries/{}", registry["id"].as_str().unwrap()),
            Some(&token),
            r#"{"confirm_name":"wrong"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(
        wrong_registry_confirmation.status(),
        StatusCode::BAD_REQUEST
    );

    let webhook = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/v1/projects/{project_id}/webhooks"),
            Some(&token),
            r#"{"name":"deploy","url":"https://hooks.example.com/deploy","secret":"secret"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(webhook.status(), StatusCode::CREATED);
    let webhook = webhook.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&webhook).contains("secret_configured"));
    assert!(!String::from_utf8_lossy(&webhook).contains("secret\""));
    let webhook: serde_json::Value = serde_json::from_slice(&webhook).unwrap();
    let wrong_webhook_confirmation = app
        .clone()
        .oneshot(request(
            "DELETE",
            &format!("/api/v1/webhooks/{}", webhook["id"].as_str().unwrap()),
            Some(&token),
            r#"{"confirm_name":"wrong"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(wrong_webhook_confirmation.status(), StatusCode::BAD_REQUEST);

    let service = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/v1/projects/{project_id}/services"),
            Some(&token),
            r#"{"name":"web","image_reference":"nginx@sha256:deadbeef","internal_port":8080,"healthcheck":null,"variables":[]}"#,
        ))
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let service: serde_json::Value = serde_json::from_slice(&service).unwrap();
    let terminal = app
        .oneshot(request(
            "GET",
            &format!(
                "/api/v1/services/{}/terminal/capability",
                service["id"].as_str().unwrap()
            ),
            Some(&token),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(terminal.status(), StatusCode::OK);
    let terminal = terminal.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&terminal).contains("\"available\":false"));
}

#[tokio::test]
async fn runtime_status_requires_auth_and_reports_component_state() {
    let mut state = state().await;
    state.runtime_health = Arc::new(StaticRuntimeHealth(false));
    let token = session_token(&state).await;
    let app = router(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.secure_cookies,
        state.trusted_origins.clone(),
    );

    let unauthenticated = app
        .clone()
        .oneshot(request("GET", "/api/v1/runtime/status", None, ""))
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let response = app
        .oneshot(request("GET", "/api/v1/runtime/status", Some(&token), ""))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let status: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(status["database"], "ready");
    assert_eq!(status["runtime"], "unavailable");
    assert_eq!(status["worker"], "ready");
    assert!(status["metrics"].is_null());
}

#[tokio::test]
async fn runtime_status_returns_available_host_metrics() {
    struct MetricsHealth;

    impl ignitify_control_plane::RuntimeHealth for MetricsHealth {
        fn ready(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
            Box::pin(std::future::ready(true))
        }

        fn host_metrics(
            &self,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Option<HostRuntimeMetrics>> + Send + '_>,
        > {
            Box::pin(std::future::ready(Some(HostRuntimeMetrics {
                containers: 4,
                containers_running: 2,
                images: 7,
                cpus: 8,
                memory_bytes: 16,
            })))
        }
    }

    let mut state = state().await;
    state.runtime_health = Arc::new(MetricsHealth);
    let token = session_token(&state).await;
    let app = router(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.secure_cookies,
        state.trusted_origins.clone(),
    );

    let response = app
        .oneshot(request("GET", "/api/v1/runtime/status", Some(&token), ""))
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let status: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(status["metrics"]["containers_running"], 2);
}
