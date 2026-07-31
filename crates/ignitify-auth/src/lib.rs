//! Authentication service for Ignitify HTTP adapters.

use std::sync::Arc;

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use ignitify_db::{Database, DatabaseError, RotateRefreshTokenOutcome, UserRecord};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    User,
}

impl From<ignitify_db::UserRole> for UserRole {
    fn from(role: ignitify_db::UserRole) -> Self {
        match role {
            ignitify_db::UserRole::Admin => Self::Admin,
            ignitify_db::UserRole::User => Self::User,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedUser {
    pub id: String,
    pub username: String,
    pub role: UserRole,
    pub tenant_id: Option<String>,
    pub api_key_id: Option<String>,
    pub scopes: Vec<String>,
}

impl AuthenticatedUser {
    pub fn has_admin_access(&self) -> bool {
        matches!(self.role, UserRole::Admin)
            && (self.api_key_id.is_none()
                || self.scopes.iter().any(|scope| scope.trim() == "admin:*"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub access_token: String,
    pub token_type: String,
    pub expires_at: DateTime<Utc>,
    pub user: AuthenticatedUser,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub access_token_ttl_minutes: i64,
    pub refresh_token_ttl_days: i64,
    pub refresh_absolute_ttl_days: i64,
    pub api_key_prefix: String,
    pub secure_cookies: bool,
    pub trusted_origins: Vec<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "change-me-in-production".to_owned(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
            refresh_absolute_ttl_days: 30,
            api_key_prefix: "ignitify_".to_owned(),
            secure_cookies: false,
            trusted_origins: vec![
                "http://localhost:6565".to_owned(),
                "http://127.0.0.1:6565".to_owned(),
            ],
        }
    }
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("bootstrap has already been completed")]
    AlreadyBootstrapped,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("user is inactive")]
    InactiveUser,
    #[error("invalid token")]
    InvalidToken,
    #[error("invalid request")]
    InvalidRequest,
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    PasswordHash(#[from] argon2::password_hash::Error),
    #[error(transparent)]
    Jwt(#[from] jsonwebtoken::errors::Error),
}

pub type Result<T> = std::result::Result<T, AuthError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Claims {
    sub: String,
    username: String,
    role: UserRole,
    auth_version: i64,
    session_family_id: String,
    exp: usize,
    iat: usize,
}

#[derive(Clone)]
pub struct AuthService {
    database: Database,
    config: AuthConfig,
}

impl AuthService {
    pub fn new(database: Database, config: AuthConfig) -> Self {
        Self { database, config }
    }

    pub fn shared(self) -> Arc<Self> {
        Arc::new(self)
    }

    pub async fn bootstrap_required(&self) -> Result<bool> {
        Ok(self.database.users().count().await? == 0)
    }

    pub async fn bootstrap_admin(&self, username: &str, password: &str) -> Result<AuthSession> {
        let username = validate_credentials(username, password)?;
        let password_hash = hash_password(password)?;
        let Some(user) = self
            .database
            .users()
            .bootstrap_admin(username, &password_hash)
            .await?
        else {
            return Err(AuthError::AlreadyBootstrapped);
        };
        self.database.users().set_last_login(&user.id).await?;
        self.database
            .users()
            .audit(&user.id, "auth.bootstrap")
            .await?;
        self.issue_session(user).await
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<AuthSession> {
        let username = validate_credentials(username, password)?;
        let Some(user) = self.database.users().get_by_username(username).await? else {
            return Err(AuthError::InvalidCredentials);
        };
        if !verify_password(&user.password_hash, password) {
            return Err(AuthError::InvalidCredentials);
        }
        ensure_active(&user)?;
        self.database.users().set_last_login(&user.id).await?;
        self.database.users().audit(&user.id, "auth.login").await?;
        self.issue_session(user).await
    }

    pub async fn refresh_session(&self, plaintext: &str) -> Result<AuthSession> {
        let successor = generate_token();
        let outcome = self
            .database
            .refresh_tokens()
            .rotate(
                &hash_token(plaintext.trim()),
                &hash_token(&successor),
                Duration::days(self.config.refresh_token_ttl_days),
            )
            .await?;
        let record = match outcome {
            RotateRefreshTokenOutcome::Rotated(record) => record,
            RotateRefreshTokenOutcome::Reused { user_id, family_id } => {
                self.database
                    .refresh_tokens()
                    .revoke_family(&user_id, &family_id)
                    .await?;
                return Err(AuthError::InvalidToken);
            }
            RotateRefreshTokenOutcome::Missing | RotateRefreshTokenOutcome::Expired => {
                return Err(AuthError::InvalidToken);
            }
        };
        let Some(user) = self.database.users().get_by_id(&record.user_id).await? else {
            self.database
                .refresh_tokens()
                .revoke_family(&record.user_id, &record.family_id)
                .await?;
            return Err(AuthError::InvalidToken);
        };
        ensure_active(&user)?;
        self.issue_rotated_session(user, record.family_id, record.expires_at, successor)
    }

    pub async fn revoke_refresh_token(&self, plaintext: &str) -> Result<()> {
        self.database
            .refresh_tokens()
            .revoke_family_by_hash(&hash_token(plaintext.trim()))
            .await?;
        Ok(())
    }

    pub async fn authenticate_bearer(&self, token: &str) -> Result<AuthenticatedUser> {
        let claims = self.decode_claims(token)?;
        let Some(user) = self.database.users().get_by_id(&claims.sub).await? else {
            return Err(AuthError::InvalidToken);
        };
        ensure_active(&user)?;
        if user.auth_version != claims.auth_version
            || !self
                .database
                .refresh_tokens()
                .has_live_family(&user.id, &claims.session_family_id)
                .await?
        {
            return Err(AuthError::InvalidToken);
        }
        Ok(authenticated_user(&user))
    }

    async fn issue_session(&self, user: UserRecord) -> Result<AuthSession> {
        let refresh_token = generate_token();
        let record = self
            .database
            .refresh_tokens()
            .create(
                &user.id,
                &hash_token(&refresh_token),
                Duration::days(self.config.refresh_token_ttl_days),
                Duration::days(self.config.refresh_absolute_ttl_days),
            )
            .await?;
        self.issue_rotated_session(user, record.family_id, record.expires_at, refresh_token)
    }

    fn issue_rotated_session(
        &self,
        user: UserRecord,
        family_id: String,
        refresh_expires_at: DateTime<Utc>,
        refresh_token: String,
    ) -> Result<AuthSession> {
        let expires_at = Utc::now() + Duration::minutes(self.config.access_token_ttl_minutes);
        let claims = Claims {
            sub: user.id.clone(),
            username: user.username.clone(),
            role: user.role.clone().into(),
            auth_version: user.auth_version,
            session_family_id: family_id,
            exp: expires_at.timestamp() as usize,
            iat: Utc::now().timestamp() as usize,
        };
        let access_token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )?;
        Ok(AuthSession {
            access_token,
            token_type: "Bearer".to_owned(),
            expires_at,
            user: authenticated_user(&user),
            refresh_token: Some(refresh_token),
            refresh_token_expires_at: Some(refresh_expires_at),
        })
    }

    fn decode_claims(&self, token: &str) -> Result<Claims> {
        decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.config.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map(|decoded| decoded.claims)
        .map_err(|_| AuthError::InvalidToken)
    }
}

fn validate_credentials<'a>(username: &'a str, password: &str) -> Result<&'a str> {
    let username = username.trim();
    if username.is_empty() || username.len() > 64 || !(8..=1024).contains(&password.len()) {
        return Err(AuthError::InvalidCredentials);
    }
    Ok(username)
}

fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}

fn verify_password(password_hash: &str, password: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(password_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

fn generate_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn hash_token(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn authenticated_user(user: &UserRecord) -> AuthenticatedUser {
    AuthenticatedUser {
        id: user.id.clone(),
        username: user.username.clone(),
        role: user.role.clone().into(),
        tenant_id: None,
        api_key_id: None,
        scopes: Vec::new(),
    }
}

fn ensure_active(user: &UserRecord) -> Result<()> {
    if user.is_active {
        Ok(())
    } else {
        Err(AuthError::InactiveUser)
    }
}

#[cfg(test)]
mod tests {
    use ignitify_db::DatabaseConfig;

    use super::{AuthConfig, AuthError, AuthService};

    async fn service() -> AuthService {
        let database = ignitify_db::Database::connect(&DatabaseConfig {
            url: "sqlite::memory:".to_owned(),
        })
        .await
        .unwrap();
        AuthService::new(
            database,
            AuthConfig {
                jwt_secret: "test-secret".to_owned(),
                ..AuthConfig::default()
            },
        )
    }

    #[tokio::test]
    async fn refresh_reuse_revokes_token_family() {
        let service = service().await;
        let session = service
            .bootstrap_admin("admin", "password123")
            .await
            .unwrap();
        let old_refresh = session.refresh_token.unwrap();
        let successor = service
            .refresh_session(&old_refresh)
            .await
            .unwrap()
            .refresh_token
            .unwrap();
        let _ = service.refresh_session(&old_refresh).await;

        assert!(matches!(
            service.refresh_session(&successor).await,
            Err(AuthError::InvalidToken)
        ));
    }
}
