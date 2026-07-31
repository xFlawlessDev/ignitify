use std::{env, sync::Arc};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use ignitify_auth::{
    AuthConfig, AuthError, AuthService, AuthSession, AuthenticatedUser, BootstrapRequest,
    LoginRequest,
};
use ignitify_db::{Database, DatabaseConfig};
use serde::Serialize;
use tokio::net::TcpListener;

const REFRESH_COOKIE: &str = "ignitify_refresh";
const API_PREFIX: &str = "/api/v1";

#[derive(Clone)]
struct AppState {
    auth: Arc<AuthService>,
    secure_cookies: bool,
}

#[derive(Serialize)]
struct BootstrapStatus {
    required: bool,
}

#[derive(Serialize)]
struct MessageResponse {
    message: &'static str,
}

#[derive(Debug)]
struct ApiError(AuthError);

impl From<AuthError> for ApiError {
    fn from(error: AuthError) -> Self {
        Self(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            AuthError::AlreadyBootstrapped => StatusCode::CONFLICT,
            AuthError::InvalidCredentials | AuthError::InactiveUser | AuthError::InvalidToken => {
                StatusCode::UNAUTHORIZED
            }
            AuthError::InvalidRequest => StatusCode::BAD_REQUEST,
            AuthError::Database(_) | AuthError::PasswordHash(_) | AuthError::Jwt(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        let message = if status.is_server_error() {
            "internal server error".to_owned()
        } else {
            self.0.to_string()
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

async fn health() -> &'static str {
    "ok"
}

async fn bootstrap_status(
    State(state): State<AppState>,
) -> Result<Json<BootstrapStatus>, ApiError> {
    let required = state.auth.bootstrap_required().await?;
    Ok(Json(BootstrapStatus { required }))
}

async fn bootstrap(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BootstrapRequest>,
) -> Result<Response, ApiError> {
    require_same_origin_request(&headers)?;
    session_response(
        state.secure_cookies,
        state
            .auth
            .bootstrap_admin(&request.username, &request.password)
            .await?,
    )
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Response, ApiError> {
    require_same_origin_request(&headers)?;
    session_response(
        state.secure_cookies,
        state
            .auth
            .login(&request.username, &request.password)
            .await?,
    )
}

async fn refresh(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    require_same_origin_request(&headers)?;
    let token = refresh_cookie(&headers).ok_or(AuthError::InvalidToken)?;
    session_response(
        state.secure_cookies,
        state.auth.refresh_session(token).await?,
    )
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    require_same_origin_request(&headers)?;
    if let Some(token) = refresh_cookie(&headers) {
        state.auth.revoke_refresh_token(token).await?;
    }
    Ok(with_refresh_cookie(
        Json(MessageResponse {
            message: "logged out",
        })
        .into_response(),
        &clear_refresh_cookie(state.secure_cookies),
    ))
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthenticatedUser>, ApiError> {
    let token = bearer_token(&headers).ok_or(AuthError::InvalidToken)?;
    Ok(Json(state.auth.authenticate_bearer(token).await?))
}

fn session_response(secure_cookies: bool, mut session: AuthSession) -> Result<Response, ApiError> {
    let refresh_token = session
        .refresh_token
        .take()
        .ok_or(AuthError::InvalidToken)?;
    session.refresh_token_expires_at = None;
    Ok(with_refresh_cookie(
        Json(session).into_response(),
        &refresh_cookie_header(&refresh_token, secure_cookies),
    ))
}

fn with_refresh_cookie(mut response: Response, cookie: &str) -> Response {
    if let Ok(value) = HeaderValue::from_str(cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    response
}

fn refresh_cookie_header(token: &str, secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!(
        "{REFRESH_COOKIE}={token}; Path={API_PREFIX}/auth; HttpOnly; SameSite=Lax; Max-Age=604800{secure}"
    )
}

fn clear_refresh_cookie(secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("{REFRESH_COOKIE}=; Path={API_PREFIX}/auth; HttpOnly; SameSite=Lax; Max-Age=0{secure}")
}

fn require_same_origin_request(headers: &HeaderMap) -> Result<(), ApiError> {
    if headers.get("X-Ignitify-Request").is_some() {
        Ok(())
    } else {
        Err(AuthError::InvalidRequest.into())
    }
}

fn refresh_cookie(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|entry| entry.strip_prefix(&format!("{REFRESH_COOKIE}=")))
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
}

fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().or_else(|| match name {
        "IGNITIFY_DATABASE_URL" => option_env!("IGNITIFY_DATABASE_URL").map(str::to_owned),
        "IGNITIFY_JWT_SECRET" => option_env!("IGNITIFY_JWT_SECRET").map(str::to_owned),
        "IGNITIFY_SECURE_COOKIES" => option_env!("IGNITIFY_SECURE_COOKIES").map(str::to_owned),
        _ => None,
    })
}

fn required_env(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    env_value(name).ok_or_else(|| format!("{name} must be set").into())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database = Database::connect(&DatabaseConfig {
        url: env_value("IGNITIFY_DATABASE_URL").unwrap_or_else(|| DatabaseConfig::default().url),
    })
    .await?;
    database.ping().await?;

    let auth = AuthService::new(
        database,
        AuthConfig {
            jwt_secret: required_env("IGNITIFY_JWT_SECRET")?,
            ..AuthConfig::default()
        },
    )
    .shared();
    let state = AppState {
        auth,
        secure_cookies: env_value("IGNITIFY_SECURE_COOKIES").is_some_and(|value| value == "true"),
    };
    let app = Router::new()
        .route("/health", get(health))
        .route(
            "/api/v1/auth/bootstrap",
            get(bootstrap_status).post(bootstrap),
        )
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/refresh", post(refresh))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/me", get(me))
        .with_state(state);
    let listener = TcpListener::bind("127.0.0.1:5656").await?;

    println!("Ignitify API listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
