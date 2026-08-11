use std::collections::HashMap;
use std::{future::Future, sync::Arc};

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
use hmac::{Hmac, Mac};
use http_body_util::BodyExt;
use ignitify_auth::AuthConfig;
use ignitify_control_plane::{
    AgeCipher, ControlHandle, HostRuntimeMetrics, RuntimeContainer, RuntimePort, ServiceControl,
    StaticRuntimeHealth, StaticSystemMetrics, SystemMetricsSnapshot,
};
use ignitify_db::{
    DatabaseConfig, NewRemoteServer, ProjectActor, ServerSettingsUpdate,
    UserRole as DatabaseUserRole,
};
use ignitify_domain::ProjectMemberRole;
use sha2::{Digest, Sha256};
use tower::ServiceExt;

use crate::{DomainPolicy, router, state::AppState};

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
            bootstrap_secret: Some("test-bootstrap-secret-that-is-at-least-32-bytes".to_owned()),
            ..AuthConfig::default()
        },
    )
    .shared();
    let identity = age::x25519::Identity::generate().to_string();
    let services = ServiceControl::new(
        database.services(),
        database.projects(),
        identity.expose_secret(),
    )
    .unwrap();
    let (control, _wake) =
        ControlHandle::new(database.deployments(), identity.expose_secret()).unwrap();
    AppState {
        auth,
        database,
        services: Some(services),
        control: Some(control),
        runtime_health: Arc::new(StaticRuntimeHealth(true)),
        worker_health: Arc::new(StaticRuntimeHealth(true)),
        ingress_health: Arc::new(StaticRuntimeHealth(true)),
        system_metrics: Arc::new(StaticSystemMetrics(None)),
        docker_runtime: None,
        terminal: ignitify_terminal::TerminalService,
        host_terminal_enabled: false,
        terminal_sessions: Arc::new(tokio::sync::Semaphore::new(4)),
        login_rate_limiter: crate::state::LoginRateLimiter::default(),
        ai_chat_rate_limiter: crate::state::AiChatRateLimiter::default(),
        secure_cookies: false,
        origin_policy: crate::state::OriginPolicy::new(false, false, Arc::from([]), None),
        provider_cipher: Some(Arc::new(
            AgeCipher::from_identity(identity.expose_secret()).unwrap(),
        )),
        domain_policy: DomainPolicy::from_suffixes(["example.com".to_owned()]),
        github_manifest_states: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
    }
}

#[tokio::test]
async fn health_reports_database_ready_when_runtime_is_unavailable() {
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
        Arc::from([]),
    );

    let response = app
        .oneshot(request("GET", "/health", None, ""))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn system_metrics_returns_provider_snapshot_for_authenticated_actor() {
    let state = state().await;
    let token = session_token(&state).await;
    let app = crate::router_with_system_metrics(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        Arc::new(StaticSystemMetrics(Some(SystemMetricsSnapshot {
            cpu_usage_percentage: 42.5,
            cpu_cores: 8,
            memory_used_bytes: 4,
            memory_total_bytes: 8,
            disk_used_bytes: 10,
            disk_total_bytes: 20,
            docker_disk_used_bytes: Some(3),
            docker_disk_total_bytes: Some(5),
            block_read_bytes_per_second: 100.0,
            block_write_bytes_per_second: 50.0,
            network_receive_bytes_per_second: 200.0,
            network_transmit_bytes_per_second: 80.0,
        }))),
        ignitify_terminal::TerminalService,
        state.secure_cookies,
        Arc::from([]),
    );

    let response = app
        .oneshot(request("GET", "/api/v1/runtime/metrics", Some(&token), ""))
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["cpu_usage_percentage"], 42.5);
    assert_eq!(json["docker_disk_total_bytes"], 5);
}

#[tokio::test]
async fn uptime_monitor_api_validates_and_persists_custom_endpoints() {
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
        Arc::from([]),
    );

    let unauthenticated = app
        .clone()
        .oneshot(request("GET", "/api/v1/uptime-monitors", None, ""))
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let invalid = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/uptime-monitors",
            Some(&token),
            r#"{"name":"Private","target":"http://127.0.0.1:5656","kind":"http","interval_seconds":60,"enabled":true}"#,
        ))
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

    let created = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/uptime-monitors",
            Some(&token),
            r#"{"name":"Portal","target":"status.example.com/health","kind":"http","interval_seconds":60,"enabled":true}"#,
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body = created.into_body().collect().await.unwrap().to_bytes();
    let created_json: serde_json::Value = serde_json::from_slice(&created_body).unwrap();
    let monitor_id = created_json["id"].as_str().unwrap().to_owned();
    assert_eq!(created_json["target"], "https://status.example.com/health");
    assert_eq!(created_json["status"], "pending");

    let listed = app
        .clone()
        .oneshot(request("GET", "/api/v1/uptime-monitors", Some(&token), ""))
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = listed.into_body().collect().await.unwrap().to_bytes();
    let listed_json: serde_json::Value = serde_json::from_slice(&listed_body).unwrap();
    assert_eq!(listed_json.as_array().unwrap().len(), 1);

    let removed = app
        .oneshot(request(
            "DELETE",
            &format!("/api/v1/uptime-monitors/{monitor_id}"),
            Some(&token),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn provider_routes_encrypt_credentials_and_require_admin_mutations() {
    let state = state().await;
    let token = session_token(&state).await;
    let app = crate::router_with_system_metrics_and_docker_and_provider_cipher(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.system_metrics.clone(),
        state.docker_runtime.clone(),
        state.terminal,
        state.secure_cookies,
        Arc::from([]),
        state.provider_cipher.clone(),
    );

    let unauthenticated = app
        .clone()
        .oneshot(request("GET", "/api/v1/providers", None, ""))
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let manifest_start = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/providers/github/manifest",
            Some(&token),
            r#"{"name":"Ignitify Direct App"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(manifest_start.status(), StatusCode::OK);
    let manifest_body = manifest_start
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_body).unwrap();
    assert!(
        manifest["action_url"]
            .as_str()
            .unwrap()
            .starts_with("https://github.com/settings/apps/new?state=")
    );
    let generated_name = manifest["manifest"]["name"].as_str().unwrap();
    assert!(generated_name.starts_with("Ignitify Direct App-"));
    assert!(generated_name.len() <= 34);
    assert_eq!(
        manifest["manifest"]["default_permissions"]["contents"],
        "read"
    );
    assert!(
        manifest["manifest"]["redirect_url"]
            .as_str()
            .unwrap()
            .ends_with("/api/v1/providers/github/manifest/callback")
    );

    let created = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/providers",
            Some(&token),
            r#"{"name":"Main GitLab","kind":"gitlab","auth_mode":"oauth","base_url":"https://gitlab.example.com/","redirect_uri":"https://ignitify.example.com/api/providers/gitlab/callback","client_id":"client-id","username":"deploy","client_secret":"provider-secret"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let body = created.into_body().collect().await.unwrap().to_bytes();
    assert!(!String::from_utf8_lossy(&body).contains("provider-secret"));
    let provider: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(provider["token_configured"], true);

    let connection_test = app
        .clone()
        .oneshot(request(
            "POST",
            &format!(
                "/api/v1/providers/{}/test",
                provider["id"].as_str().unwrap()
            ),
            Some(&token),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(connection_test.status(), StatusCode::BAD_REQUEST);

    let github_app = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/providers",
            Some(&token),
            r#"{"name":"Ignitify GitHub App","kind":"github","auth_mode":"github_app","base_url":"https://github.com","application_id":"12345","installation_id":"67890","private_key":"-----BEGIN PRIVATE KEY-----\nsecret\n-----END PRIVATE KEY-----"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(github_app.status(), StatusCode::CREATED);
    let github_body = github_app.into_body().collect().await.unwrap().to_bytes();
    let github_provider: serde_json::Value = serde_json::from_slice(&github_body).unwrap();
    assert_eq!(github_provider["kind"], "github");
    assert_eq!(github_provider["auth_mode"], "github_app");
    assert!(!String::from_utf8_lossy(&github_body).contains("BEGIN PRIVATE KEY"));

    let stored = state
        .database
        .providers()
        .get(provider["id"].as_str().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_ne!(stored.credentials_ciphertext, "provider-secret");

    let duplicate = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/providers",
            Some(&token),
            r#"{"name":"Main GitLab","kind":"gitlab","auth_mode":"oauth","base_url":"https://gitlab.example.com","redirect_uri":"https://ignitify.example.com/api/providers/gitlab/callback","client_id":"client-id","client_secret":"provider-secret"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);

    let removed = app
        .oneshot(request(
            "DELETE",
            &format!("/api/v1/providers/{}", provider["id"].as_str().unwrap()),
            Some(&token),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn remote_builder_routes_encrypt_tls_material_and_require_admin() {
    let state = state().await;
    let token = session_token(&state).await;
    let app = crate::router_with_system_metrics_and_docker_and_provider_cipher(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.system_metrics.clone(),
        state.docker_runtime.clone(),
        state.terminal,
        state.secure_cookies,
        Arc::from([]),
        state.provider_cipher.clone(),
    );
    let unauthenticated = app
        .clone()
        .oneshot(request("GET", "/api/v1/remote-builders", None, ""))
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let created = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/remote-builders",
            Some(&token),
            r#"{"name":"Primary BuildKit","endpoint":"tcp://builder.example.com:1234","registry_repository":"registry.example.com/ignitify/builds","tls_server_name":"builder.example.com","ca_certificate":"-----BEGIN CERTIFICATE-----\nca\n-----END CERTIFICATE-----","client_certificate":"-----BEGIN CERTIFICATE-----\nclient\n-----END CERTIFICATE-----","client_key":"-----BEGIN PRIVATE KEY-----\nkey\n-----END PRIVATE KEY-----","is_default":true}"#,
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let body = created.into_body().collect().await.unwrap().to_bytes();
    assert!(!String::from_utf8_lossy(&body).contains("BEGIN CERTIFICATE"));
    assert!(!String::from_utf8_lossy(&body).contains("BEGIN PRIVATE KEY"));

    let list = app
        .oneshot(request("GET", "/api/v1/remote-builders", Some(&token), ""))
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let body = list.into_body().collect().await.unwrap().to_bytes();
    let builders: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(builders[0]["is_default"], true);
    assert!(builders[0].get("client_key").is_none());
}

#[tokio::test]
async fn remote_server_routes_hide_private_credentials_and_provide_access_setup() {
    let state = state().await;
    let token = session_token(&state).await;
    let app = crate::router_with_system_metrics_and_docker_and_provider_cipher(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.system_metrics.clone(),
        state.docker_runtime.clone(),
        state.terminal,
        state.secure_cookies,
        Arc::from([]),
        state.provider_cipher.clone(),
    );
    let unauthenticated = app
        .clone()
        .oneshot(request("GET", "/api/v1/remote-servers", None, ""))
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
    let unauthenticated_check = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/remote-servers/unknown/check",
            None,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(unauthenticated_check.status(), StatusCode::UNAUTHORIZED);

    let invalid_create = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/remote-servers",
            Some(&token),
            r#"{"name":"Production VM","host":"not a valid host","port":22,"username":"ignitify"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(invalid_create.status(), StatusCode::BAD_REQUEST);

    let cipher = state.provider_cipher.as_ref().unwrap();
    let created = state
        .database
        .remote_servers()
        .create(NewRemoteServer {
            name: "Production VM".to_owned(),
            host: "production.example.com".to_owned(),
            port: 22,
            username: "ignitify".to_owned(),
            deploy_path: "/srv/ignitify".to_owned(),
            private_key_ciphertext: cipher.encrypt(b"private-key").unwrap(),
            public_key_ciphertext: cipher
                .encrypt(b"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExample deploy@host")
                .unwrap(),
            known_hosts_ciphertext: cipher
                .encrypt(b"production.example.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExample")
                .unwrap(),
            is_default: true,
        })
        .await
        .unwrap();

    let servers = app
        .clone()
        .oneshot(request("GET", "/api/v1/remote-servers", Some(&token), ""))
        .await
        .unwrap();
    assert_eq!(servers.status(), StatusCode::OK);
    let body = servers.into_body().collect().await.unwrap().to_bytes();
    assert!(!String::from_utf8_lossy(&body).contains("AAAAC3Nza"));

    let access = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/v1/remote-servers/{}/access", created.id),
            Some(&token),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(access.status(), StatusCode::OK);
    let body = access.into_body().collect().await.unwrap().to_bytes();
    let access: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        access["public_key"],
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExample deploy@host"
    );

    let updated = app
        .oneshot(request(
            "PATCH",
            &format!("/api/v1/remote-servers/{}", created.id),
            Some(&token),
            r#"{"name":"Production VM","host":"production.example.com","port":2222,"username":"ignitify","deploy_path":"/srv/ignitify","is_default":true}"#,
        ))
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);
    let body = updated.into_body().collect().await.unwrap().to_bytes();
    let updated: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated["port"], 2222);
    assert_eq!(updated["known_hosts_configured"], true);
}

#[tokio::test]
async fn backup_s3_destination_routes_encrypt_credentials_and_require_admin() {
    let state = state().await;
    let token = session_token(&state).await;
    let app = crate::router_with_system_metrics_and_docker_and_provider_cipher(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.system_metrics.clone(),
        state.docker_runtime.clone(),
        state.terminal,
        state.secure_cookies,
        Arc::from([]),
        state.provider_cipher.clone(),
    );
    let unauthenticated = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/settings/backup-destination/s3",
            None,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let created = app
        .clone()
        .oneshot(request(
            "PUT",
            "/api/v1/settings/backup-destination/s3",
            Some(&token),
            r#"{"endpoint":"https://account.r2.cloudflarestorage.com","region":"auto","bucket":"ignitify-backups","prefix":"production","access_key_id":"access-key-id","secret_access_key":"secret-access-key","session_token":"session-token"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let created_body = created.into_body().collect().await.unwrap().to_bytes();
    let created_text = String::from_utf8_lossy(&created_body);
    assert!(!created_text.contains("access-key-id"));
    assert!(!created_text.contains("secret-access-key"));
    assert!(!created_text.contains("session-token"));
    assert!(created_text.contains("AES256"));

    let stored = state
        .database
        .backup_destinations()
        .s3_connection()
        .await
        .unwrap()
        .unwrap();
    assert_ne!(stored.access_key_id_ciphertext, "access-key-id");
    assert_ne!(stored.secret_access_key_ciphertext, "secret-access-key");

    let fetched = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/settings/backup-destination/s3",
            Some(&token),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);
    let fetched_body = fetched.into_body().collect().await.unwrap().to_bytes();
    let fetched_text = String::from_utf8_lossy(&fetched_body);
    assert!(fetched_text.contains("ignitify-backups"));
    assert!(!fetched_text.contains("access_key_id"));
    assert!(!fetched_text.contains("secret_access_key"));

    let controls = app
        .clone()
        .oneshot(request(
            "PATCH",
            "/api/v1/settings/backup-destination/s3",
            Some(&token),
            r#"{"enabled":false,"schedule_interval_hours":48}"#,
        ))
        .await
        .unwrap();
    assert_eq!(controls.status(), StatusCode::OK);
    let controls_body = controls.into_body().collect().await.unwrap().to_bytes();
    let controls: serde_json::Value = serde_json::from_slice(&controls_body).unwrap();
    assert_eq!(controls["enabled"], false);
    assert_eq!(controls["schedule_interval_hours"], 48);
    assert!(
        state
            .database
            .backup_destinations()
            .s3_connection()
            .await
            .unwrap()
            .is_none()
    );

    let runs = app
        .oneshot(request(
            "GET",
            "/api/v1/settings/backup-destination/s3/runs",
            Some(&token),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(runs.status(), StatusCode::OK);
    let runs_body = runs.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&runs_body).unwrap(),
        serde_json::json!([])
    );
}

#[tokio::test]
async fn openapi_document_and_swagger_ui_are_served() {
    let state = state().await;
    let app = crate::router(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.secure_cookies,
        Arc::from([]),
    );

    let document = app
        .clone()
        .oneshot(request("GET", "/api-docs/openapi.json", None, ""))
        .await
        .unwrap();
    assert_eq!(document.status(), StatusCode::OK);
    let body = document.into_body().collect().await.unwrap().to_bytes();
    let document: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(document["info"]["title"], "Ignitify API");
    assert_eq!(
        document["components"]["securitySchemes"]["bearerAuth"]["scheme"],
        "bearer"
    );
    assert_eq!(
        document["components"]["securitySchemes"]["agentBearerAuth"]["scheme"],
        "bearer"
    );
    let paths = document["paths"].as_object().unwrap();
    assert!(
        paths.len() >= 61,
        "OpenAPI document should cover every registered API path"
    );
    for path in [
        "/api/v1/auth/login",
        "/api/v1/providers/{provider_id}/repositories",
        "/api/v1/runtime/containers/{container_id}/details",
        "/api/v1/settings/infrastructure",
        "/api/v1/remote-servers/{server_id}/agent/install",
        "/api/v1/projects/{project_id}/services",
        "/api/v1/services/{service_id}/deployments",
        "/api/v1/deployments/{deployment_id}/rollback",
        "/api/v1/webhooks/services/{service_id}",
    ] {
        assert!(paths.contains_key(path), "missing OpenAPI path: {path}");
    }
    assert!(document["paths"]["/api/v1/settings/backup-destination/s3"]["put"].is_object());
    let parameters =
        document["paths"]["/api/v1/settings/backup-destination/s3"]["put"]["parameters"]
            .as_array()
            .unwrap();
    assert!(parameters.iter().any(|parameter| {
        parameter["name"] == "X-Ignitify-Request" && parameter["in"] == "header"
    }));
    let heartbeat = &document["paths"]["/api/v1/remote-agents/heartbeat"]["post"];
    assert!(heartbeat["security"][0]["agentBearerAuth"].is_array());
    assert!(heartbeat["parameters"].as_array().is_none_or(|parameters| {
        parameters.iter().all(|parameter| {
            parameter["name"] != "X-Ignitify-Request" || parameter["in"] != "header"
        })
    }));

    let swagger_ui = app
        .oneshot(request("GET", "/swagger-ui/", None, ""))
        .await
        .unwrap();
    assert_eq!(swagger_ui.status(), StatusCode::OK);
}

#[tokio::test]
async fn verified_github_push_webhook_queues_one_pinned_deployment() {
    let state = state().await;
    let token = session_token(&state).await;
    let app = crate::routes::router(state.clone());
    let provider = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/providers",
            Some(&token),
            r#"{"name":"GitHub","kind":"github","auth_mode":"token","base_url":"https://github.com","token":"provider-token"}"#,
        ))
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let provider: serde_json::Value = serde_json::from_slice(&provider).unwrap();
    let provider_id = provider["id"]
        .as_str()
        .unwrap_or_else(|| panic!("provider creation failed: {provider}"));
    let project = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/projects",
            Some(&token),
            r#"{"name":"Webhook app"}"#,
        ))
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let project: serde_json::Value = serde_json::from_slice(&project).unwrap();
    let service_body = serde_json::json!({
        "name": "web",
        "kind": "image",
        "internal_port": 8080,
        "healthcheck": null,
        "variables": [],
        "source_config": {
            "source": "application",
            "provider_id": provider_id,
            "repository": "acme/site",
            "branch": "main",
            "builder": "railpack",
            "auto_deploy": true
        }
    });
    let service = app
        .clone()
        .oneshot(request(
            "POST",
            &format!(
                "/api/v1/projects/{}/services",
                project["id"].as_str().unwrap()
            ),
            Some(&token),
            &service_body.to_string(),
        ))
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let service: serde_json::Value = serde_json::from_slice(&service).unwrap();
    let service_id = service["id"]
        .as_str()
        .unwrap_or_else(|| panic!("service creation failed: {service}"));
    let secret = service["auto_deploy_webhook_secret"]
        .as_str()
        .unwrap_or_else(|| panic!("service response omitted auto-deploy secret: {service}"));
    let body = br#"{"ref":"refs/heads/main","after":"0123456789abcdef0123456789abcdef01234567","repository":{"full_name":"acme/site"}}"#;
    let mut invalid = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/webhooks/services/{service_id}"))
        .header("content-type", "application/json")
        .header("x-github-event", "push")
        .header("x-hub-signature-256", "sha256:invalid")
        .body(Body::from(body.as_slice().to_vec()))
        .unwrap();
    invalid
        .headers_mut()
        .insert("x-github-delivery", "delivery-1".parse().unwrap());
    assert_eq!(
        app.clone().oneshot(invalid).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );

    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body);
    let signature = format!("sha256:{:x}", mac.finalize().into_bytes());
    let mut valid = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/webhooks/services/{service_id}"))
        .header("content-type", "application/json")
        .header("x-github-event", "push")
        .header("x-hub-signature-256", signature)
        .header("x-github-delivery", "delivery-1")
        .body(Body::from(body.as_slice().to_vec()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(valid).await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    valid = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/webhooks/services/{service_id}"))
        .header("content-type", "application/json")
        .header("x-github-event", "push")
        .header("x-hub-signature-256", {
            let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
            mac.update(body);
            format!("sha256:{:x}", mac.finalize().into_bytes())
        })
        .header("x-github-delivery", "delivery-1")
        .body(Body::from(body.as_slice().to_vec()))
        .unwrap();
    assert_eq!(
        app.oneshot(valid).await.unwrap().status(),
        StatusCode::NO_CONTENT
    );

    let deployments = state
        .database
        .deployments()
        .list(
            ignitify_db::DeploymentActor {
                id: "owner",
                is_admin: true,
            },
            service_id,
            None,
            None,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(deployments.len(), 1);
    assert_eq!(
        deployments[0].source_revision.as_deref(),
        Some("0123456789abcdef0123456789abcdef01234567")
    );
}

async fn session_token(state: &AppState) -> String {
    state
        .auth
        .bootstrap_admin(
            "test-bootstrap-secret-that-is-at-least-32-bytes",
            "owner",
            "password123",
        )
        .await
        .unwrap()
        .access_token
}

#[tokio::test]
async fn remote_agent_heartbeat_authenticates_and_persists_metrics() {
    let state = state().await;
    let server = state
        .database
        .remote_servers()
        .create(NewRemoteServer {
            name: "Agent VM".to_owned(),
            host: "agent.example.com".to_owned(),
            port: 22,
            username: "ignitify".to_owned(),
            deploy_path: "/srv/ignitify".to_owned(),
            private_key_ciphertext: "private".to_owned(),
            public_key_ciphertext: "public".to_owned(),
            known_hosts_ciphertext: "known-hosts".to_owned(),
            is_default: true,
        })
        .await
        .unwrap();
    let token = "agent-test-token";
    let hash = format!("{:x}", Sha256::digest(token.as_bytes()));
    state
        .database
        .remote_server_agents()
        .install(&server.id, &hash)
        .await
        .unwrap();
    let app = crate::routes::router(state.clone());
    let mut heartbeat = request(
        "POST",
        "/api/v1/remote-agents/heartbeat",
        None,
        &format!(
            r#"{{"server_id":"{}","version":"0.1.0","cpu_cores":2,"memory_used_bytes":10,"memory_total_bytes":20}}"#,
            server.id
        ),
    );
    heartbeat
        .headers_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    let response = app.oneshot(heartbeat).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let agent = state
        .database
        .remote_server_agents()
        .get(&server.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(agent.status, "online");
    assert_eq!(agent.memory_total_bytes, Some(20));
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
async fn infrastructure_settings_require_admin_and_persist_validated_updates() {
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
        Arc::from([]),
    );

    let unauthenticated = app
        .clone()
        .oneshot(request("GET", "/api/v1/settings/infrastructure", None, ""))
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let password_hash = Argon2::default()
        .hash_password(b"password123", &SaltString::generate(&mut OsRng))
        .unwrap()
        .to_string();
    state
        .database
        .users()
        .create("settings-user", &password_hash, DatabaseUserRole::User)
        .await
        .unwrap();
    let user_token = state
        .auth
        .login("settings-user", "password123")
        .await
        .unwrap()
        .access_token;
    let forbidden = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/settings/infrastructure",
            Some(&user_token),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let updated = app
        .clone()
        .oneshot(request(
            "PATCH",
            "/api/v1/settings/infrastructure",
            Some(&token),
            r#"{"application_domain_suffix":"Apps.Example.com","https_enabled":true,"automatically_provision_ssl":true,"acme_email":"ops@apps.example.com","dns_record_type":"a","dns_record_target":"203.0.113.10","fallback_page_heading":"This application is unavailable","fallback_page_message":"Check the address and try again.","certificate_provider":"lets-encrypt","custom_certificate_id":null,"concurrent_builds":4}"#,
        ))
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);
    let body = updated.into_body().collect().await.unwrap().to_bytes();
    let settings: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(settings["application_domain_suffix"], "apps.example.com");
    assert_eq!(settings["acme_email"], "ops@apps.example.com");
    assert_eq!(settings["dns_record_type"], "a");
    assert_eq!(settings["dns_record_target"], "203.0.113.10");
    assert_eq!(
        settings["fallback_page_heading"],
        "This application is unavailable"
    );
    assert_eq!(
        settings["fallback_page_message"],
        "Check the address and try again."
    );
    assert_eq!(settings["certificate_provider"], "lets-encrypt");
    assert_eq!(settings["application"]["public_origin"], "");
    assert_eq!(settings["application"]["secure_cookies"], false);
    assert_eq!(settings["health"]["database"], "ready");
    assert_eq!(settings["concurrent_builds"], 4);

    let persisted = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/settings/infrastructure",
            Some(&token),
            "",
        ))
        .await
        .unwrap();
    let body = persisted.into_body().collect().await.unwrap().to_bytes();
    let settings: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(settings["application_domain_suffix"], "apps.example.com");
    assert_eq!(
        settings["fallback_page_heading"],
        "This application is unavailable"
    );

    let invalid = app
        .clone()
        .oneshot(request(
            "PATCH",
            "/api/v1/settings/infrastructure",
            Some(&token),
            r#"{"application_domain_suffix":"apps.example.com","https_enabled":true,"automatically_provision_ssl":true,"acme_email":"ops@apps.example.com","dns_record_type":"a","dns_record_target":"203.0.113.10","certificate_provider":"custom","custom_certificate_id":null}"#,
        ))
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

    let invalid_email = app
        .clone()
        .oneshot(request(
            "PATCH",
            "/api/v1/settings/infrastructure",
            Some(&token),
            r#"{"application_domain_suffix":"apps.example.com","https_enabled":true,"automatically_provision_ssl":true,"acme_email":"not-an-email","dns_record_type":"a","dns_record_target":"203.0.113.10","certificate_provider":"lets-encrypt","custom_certificate_id":null}"#,
        ))
        .await
        .unwrap();
    assert_eq!(invalid_email.status(), StatusCode::BAD_REQUEST);

    let invalid_fallback = app
        .oneshot(request(
            "PATCH",
            "/api/v1/settings/infrastructure",
            Some(&token),
            r#"{"application_domain_suffix":"apps.example.com","https_enabled":true,"automatically_provision_ssl":true,"acme_email":"ops@apps.example.com","dns_record_type":"a","dns_record_target":"203.0.113.10","fallback_page_heading":"","fallback_page_message":"Check the address and try again.","certificate_provider":"lets-encrypt","custom_certificate_id":null}"#,
        ))
        .await
        .unwrap();
    assert_eq!(invalid_fallback.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn control_plane_domain_enables_its_https_origin_after_a_validated_update() {
    let state = state().await;
    let token = session_token(&state).await;
    let app = router(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        true,
        Arc::from([]),
    );

    let response = app
        .oneshot(request(
            "PATCH",
            "/api/v1/settings/infrastructure",
            Some(&token),
            r#"{"control_plane_domain":"Console.Example.com","application_domain_suffix":"apps.example.com","https_enabled":true,"automatically_provision_ssl":true,"acme_email":"ops@example.com","dns_record_type":"a","dns_record_target":"203.0.113.10","certificate_provider":"lets-encrypt","custom_certificate_id":null}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let settings: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(settings["control_plane_domain"], "console.example.com");
    assert_eq!(
        settings["application"]["public_origin"],
        "https://console.example.com"
    );
}

#[tokio::test]
async fn ai_settings_encrypt_the_api_key_and_chat_requires_configuration() {
    let state = state().await;
    let token = session_token(&state).await;
    let app = crate::routes::router(state.clone());

    let unauthenticated = app
        .clone()
        .oneshot(request("GET", "/api/v1/settings/ai", None, ""))
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let unavailable = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/ai/chat",
            Some(&token),
            r#"{"messages":[{"role":"user","content":"Explain this deployment failure"}]}"#,
        ))
        .await
        .unwrap();
    assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);

    let saved = app
        .clone()
        .oneshot(request(
            "PUT",
            "/api/v1/settings/ai",
            Some(&token),
            r#"{"enabled":true,"base_url":"https://api.openai.com/v1","model":"gpt-4.1-mini","api_key":"test-ai-key","clear_api_key":false}"#,
        ))
        .await
        .unwrap();
    assert_eq!(saved.status(), StatusCode::OK);
    let saved_body = saved.into_body().collect().await.unwrap().to_bytes();
    let saved_json: serde_json::Value = serde_json::from_slice(&saved_body).unwrap();
    assert_eq!(saved_json["api_key_configured"], true);
    assert!(saved_json.get("api_key").is_none());
    assert!(!String::from_utf8_lossy(&saved_body).contains("test-ai-key"));

    let connection = state.database.ai_settings().connection().await.unwrap();
    assert_ne!(
        connection.api_key_ciphertext.as_deref(),
        Some("test-ai-key")
    );

    let read = app
        .oneshot(request("GET", "/api/v1/settings/ai", Some(&token), ""))
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::OK);
    let read_body = read.into_body().collect().await.unwrap().to_bytes();
    assert!(!String::from_utf8_lossy(&read_body).contains("test-ai-key"));
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
        Arc::from([]),
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
async fn project_environment_routes_encrypt_values_and_enforce_roles() {
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
        Arc::from([]),
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
    let project_id = project["id"].as_str().unwrap();
    let updated = app
        .clone()
        .oneshot(request(
            "PUT",
            &format!("/api/v1/projects/{project_id}/environment"),
            Some(&token),
            r#"{"variables":[{"key":"APP_ENV","value":"production","is_secret":false},{"key":"TOKEN","value":"project-secret","is_secret":true}]}"#,
        ))
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);
    let updated_body = updated.into_body().collect().await.unwrap().to_bytes();
    let updated_json: serde_json::Value = serde_json::from_slice(&updated_body).unwrap();
    assert_eq!(updated_json["variables"][0]["value"], "production");
    assert!(updated_json["variables"][1].get("value").is_none());
    assert!(!String::from_utf8_lossy(&updated_body).contains("project-secret"));

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
            &format!("/api/v1/projects/{project_id}/environment"),
            Some(&viewer_token),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(viewer_read.status(), StatusCode::OK);
    let viewer_body = viewer_read.into_body().collect().await.unwrap().to_bytes();
    let viewer_json: serde_json::Value = serde_json::from_slice(&viewer_body).unwrap();
    assert!(
        viewer_json["variables"]
            .as_array()
            .unwrap()
            .iter()
            .all(|variable| variable.get("value").is_none())
    );
    let viewer_update = app
        .oneshot(request(
            "PUT",
            &format!("/api/v1/projects/{project_id}/environment"),
            Some(&viewer_token),
            r#"{"variables":[]}"#,
        ))
        .await
        .unwrap();
    assert_eq!(viewer_update.status(), StatusCode::FORBIDDEN);
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
        Arc::from([]),
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
            r#"{"name":"web","image_reference":"nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","internal_port":null,"healthcheck":null,"variables":[]}"#,
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
        Arc::from([]),
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
            r#"{"name":"web","image_reference":"nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","internal_port":8080,"healthcheck":null,"variables":[]}"#,
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
    state
        .database
        .server_settings()
        .update(ServerSettingsUpdate {
            control_plane_domain: String::new(),
            application_domain_suffix: "apps.example.com".to_owned(),
            https_enabled: true,
            automatically_provision_ssl: true,
            acme_email: "ops@apps.example.com".to_owned(),
            dns_record_type: "a".to_owned(),
            dns_record_target: "203.0.113.10".to_owned(),
            fallback_page_heading: "Application not found".to_owned(),
            fallback_page_message:
                "The requested hostname is not connected to an active application.".to_owned(),
            certificate_provider: "lets-encrypt".to_owned(),
            custom_certificate_id: None,
            concurrent_builds: 2,
        })
        .await
        .unwrap();
    let app = crate::router_with_system_metrics_and_docker_and_provider_cipher_and_ingress_and_domain_policy(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.system_metrics.clone(),
        state.docker_runtime.clone(),
        state.terminal,
        state.host_terminal_enabled,
        false,
        false,
        state.secure_cookies,
        Arc::from([]),
        state.provider_cipher.clone(),
        state.ingress_health.clone(),
        state.domain_policy.clone(),
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
            r#"{"name":"web","image_reference":"nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","internal_port":8080,"healthcheck":null,"variables":[]}"#,
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
            r#"{"hostname":"app.apps.example.com"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::ACCEPTED);
    let body = created.into_body().collect().await.unwrap().to_bytes();
    let domain: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(domain["dns_record_type"], "a");
    assert_eq!(domain["dns_record_target"], "203.0.113.10");
    assert_eq!(domain["dns_status"], "not_checked");
    let allowed_custom_domain = app
        .clone()
        .oneshot(request(
            "POST",
            &format!(
                "/api/v1/services/{}/domains",
                service["id"].as_str().unwrap()
            ),
            Some(&token),
            r#"{"hostname":"app.other.example.com"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(allowed_custom_domain.status(), StatusCode::ACCEPTED);
    let updated_suffix = app
        .clone()
        .oneshot(request(
            "PATCH",
            "/api/v1/settings/infrastructure",
            Some(&token),
            r#"{"application_domain_suffix":"other.example.com","https_enabled":true,"automatically_provision_ssl":true,"acme_email":"ops@apps.example.com","dns_record_type":"a","dns_record_target":"203.0.113.10","certificate_provider":"lets-encrypt","custom_certificate_id":null}"#,
        ))
        .await
        .unwrap();
    assert_eq!(updated_suffix.status(), StatusCode::OK);
    let verification = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/v1/domains/{}/verify", domain["id"].as_str().unwrap()),
            Some(&token),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(verification.status(), StatusCode::OK);
    let verification = verification.into_body().collect().await.unwrap().to_bytes();
    let verification: serde_json::Value = serde_json::from_slice(&verification).unwrap();
    assert_eq!(verification["dns_status"], "pending");
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
            r#"{"confirm_hostname":"app.apps.example.com"}"#,
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
        Arc::from([]),
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
            r#"{"name":"web","image_reference":"nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","internal_port":8080,"healthcheck":["/health"],"variables":[{"key":"TOKEN","value":"plain-secret","is_secret":true},{"key":"PORT","value":"8080","is_secret":false}]}"#,
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
            r#"{"name":"web","image_reference":"nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","internal_port":8080,"healthcheck":null,"variables":[]}"#,
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
        Arc::from([]),
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
            r#"{"name":"web","image_reference":"nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","internal_port":8080,"healthcheck":null,"variables":[{"key":"TOKEN","value":"plain-secret","is_secret":true}]}"#,
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
async fn deferred_feature_routes_are_not_registered() {
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
        Arc::from([]),
    );

    for uri in [
        "/api/v1/registries",
        "/api/v1/projects/not-a-project/webhooks",
        "/api/v1/services/not-a-service/terminal/capability",
    ] {
        let response = app
            .clone()
            .oneshot(request("GET", uri, Some(&token), ""))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
    }
}

#[tokio::test]
async fn service_and_deployment_routes_fail_closed_without_capability() {
    let mut state = state().await;
    state.services = None;
    state.control = None;
    let token = session_token(&state).await;
    let app = router(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.secure_cookies,
        Arc::from([]),
    );

    let project = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/projects",
            Some(&token),
            r#"{"name":"Base"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(project.status(), StatusCode::CREATED);
    let service = app
        .oneshot(request(
            "GET",
            "/api/v1/projects/not-a-project/services",
            Some(&token),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(service.status(), StatusCode::SERVICE_UNAVAILABLE);
}
#[tokio::test]
async fn runtime_status_requires_auth_and_reports_component_state() {
    let mut state = state().await;
    state.runtime_health = Arc::new(StaticRuntimeHealth(false));
    state.ingress_health = Arc::new(StaticRuntimeHealth(false));
    let token = session_token(&state).await;
    let app = crate::router_with_system_metrics_and_docker_and_provider_cipher_and_ingress(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.system_metrics.clone(),
        state.docker_runtime.clone(),
        state.terminal,
        state.secure_cookies,
        Arc::from([]),
        state.provider_cipher.clone(),
        state.ingress_health.clone(),
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
    assert_eq!(status["ingress"], "unavailable");
    assert!(status["metrics"].is_null());
}

#[tokio::test]
async fn runtime_status_returns_available_host_metrics() {
    struct MetricsHealth;

    impl ignitify_control_plane::RuntimeHealth for MetricsHealth {
        fn ready(&self) -> std::pin::Pin<Box<dyn Future<Output = bool> + Send + '_>> {
            Box::pin(std::future::ready(true))
        }

        fn host_metrics(
            &self,
        ) -> std::pin::Pin<Box<dyn Future<Output = Option<HostRuntimeMetrics>> + Send + '_>>
        {
            Box::pin(std::future::ready(Some(HostRuntimeMetrics {
                containers: 4,
                containers_running: 3,
                images: 9,
                cpus: 2,
                memory_bytes: 1_073_741_824,
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
        Arc::from([]),
    );

    let response = app
        .oneshot(request("GET", "/api/v1/runtime/status", Some(&token), ""))
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let status: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(status["metrics"]["containers_running"], 3);
}

#[tokio::test]
async fn runtime_containers_requires_auth_and_returns_inventory() {
    struct InventoryHealth;

    impl ignitify_control_plane::RuntimeHealth for InventoryHealth {
        fn ready(&self) -> std::pin::Pin<Box<dyn Future<Output = bool> + Send + '_>> {
            Box::pin(std::future::ready(true))
        }

        fn container_inventory(
            &self,
        ) -> std::pin::Pin<Box<dyn Future<Output = Option<Vec<RuntimeContainer>>> + Send + '_>>
        {
            Box::pin(std::future::ready(Some(vec![RuntimeContainer {
                id: "f0f0f0f0f0f0f0f0".to_owned(),
                name: "web".to_owned(),
                image: "nginx:latest".to_owned(),
                state: "running".to_owned(),
                status: "Up 2 minutes".to_owned(),
                health: Some("healthy".to_owned()),
                ports: vec![RuntimePort {
                    container_port: 80,
                    host_ip: Some("0.0.0.0".to_owned()),
                    host_port: Some(8080),
                    protocol: "tcp".to_owned(),
                }],
                restart_count: 2,
                cpu_percentage: Some(1.25),
                memory_usage_bytes: Some(67_108_864),
                cpu_limit_nano_cpus: Some(1_000_000_000),
                memory_limit_bytes: Some(536_870_912),
                managed: true,
            }])))
        }
    }

    let mut state = state().await;
    state.runtime_health = Arc::new(InventoryHealth);
    let token = session_token(&state).await;
    let app = router(
        state.auth.clone(),
        state.database.clone(),
        state.services.clone(),
        state.control.clone(),
        state.runtime_health.clone(),
        state.worker_health.clone(),
        state.secure_cookies,
        Arc::from([]),
    );

    let unauthenticated = app
        .clone()
        .oneshot(request("GET", "/api/v1/runtime/containers", None, ""))
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let response = app
        .oneshot(request(
            "GET",
            "/api/v1/runtime/containers",
            Some(&token),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let inventory: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(inventory["containers"][0]["name"], "web");
    assert_eq!(inventory["containers"][0]["status"], "Up 2 minutes");
    assert_eq!(inventory["containers"][0]["state"], "running");
    assert_eq!(inventory["containers"][0]["ports"][0]["host_port"], 8080);
    assert_eq!(inventory["containers"][0]["ports"][0]["container_port"], 80);
    assert_eq!(inventory["containers"][0]["health"], "healthy");
    assert_eq!(inventory["containers"][0]["cpu_percentage"], 1.25);
    assert_eq!(inventory["containers"][0]["memory_usage_bytes"], 67_108_864);
}

#[tokio::test]
async fn runtime_container_action_requires_auth_and_docker_capability() {
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
        Arc::from([]),
    );

    let unauthenticated = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/runtime/containers/web/details",
            None,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let unavailable = app
        .oneshot(request(
            "GET",
            "/api/v1/runtime/containers/web/details",
            Some(&token),
            "",
        ))
        .await
        .unwrap();
    assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
}
