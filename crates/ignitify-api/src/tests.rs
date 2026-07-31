use std::sync::Arc;

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use ignitify_auth::AuthConfig;
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
    AppState {
        auth,
        database,
        secure_cookies: false,
        trusted_origins: Arc::from([]),
    }
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
