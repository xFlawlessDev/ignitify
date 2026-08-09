use chrono::Utc;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::{Result, UserRecord, UserRole};

#[derive(Debug, Clone, Default)]
pub struct AuditContext {
    pub source_ip: Option<String>,
    pub session_family_id: Option<String>,
    pub request_id: Option<String>,
    pub user_agent: Option<String>,
    pub outcome: AuditOutcome,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum AuditOutcome {
    #[default]
    Success,
    Failure,
}

impl AuditOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
        }
    }
}

#[derive(Debug, Clone)]
pub struct UsersRepository {
    pool: SqlitePool,
}

impl UsersRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn count(&self) -> Result<i64> {
        Ok(sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn bootstrap_admin(
        &self,
        username: &str,
        password_hash: &str,
    ) -> Result<Option<UserRecord>> {
        let id = Uuid::new_v4().to_string();
        let created_at = Utc::now().to_rfc3339();
        let created = sqlx::query(
            "INSERT INTO users (id, username, password_hash, role, is_active, created_at) SELECT ?, ?, ?, 'admin', 1, ? WHERE NOT EXISTS (SELECT 1 FROM users)",
        )
        .bind(&id)
        .bind(username)
        .bind(password_hash)
        .bind(created_at)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if created == 0 {
            return Ok(None);
        }
        self.get_by_id(&id).await
    }

    pub async fn create(
        &self,
        username: &str,
        password_hash: &str,
        role: UserRole,
    ) -> Result<UserRecord> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, role, is_active, created_at) VALUES (?, ?, ?, ?, 1, ?)",
        )
        .bind(&id)
        .bind(username)
        .bind(password_hash)
        .bind(role.as_str())
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        self.get_by_id(&id)
            .await?
            .ok_or(sqlx::Error::RowNotFound.into())
    }

    pub async fn get_by_username(&self, username: &str) -> Result<Option<UserRecord>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, username, password_hash, role, is_active, auth_version FROM users WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;
        row.map(UserRow::into_record).transpose()
    }

    pub async fn get_by_id(&self, id: &str) -> Result<Option<UserRecord>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, username, password_hash, role, is_active, auth_version FROM users WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(UserRow::into_record).transpose()
    }

    pub async fn set_last_login(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE users SET last_login = ? WHERE id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn audit(&self, user_id: &str, action: &str) -> Result<()> {
        self.audit_event(Some(user_id), action, None, None, &AuditContext::default())
            .await
    }

    pub async fn audit_event(
        &self,
        user_id: Option<&str>,
        action: &str,
        resource_type: Option<&str>,
        resource_id: Option<&str>,
        context: &AuditContext,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO audit_logs (id, user_id, action, resource_type, resource_id, source_ip, session_family_id, request_id, user_agent, outcome, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
            .bind(Uuid::new_v4().to_string())
            .bind(user_id)
            .bind(action)
            .bind(resource_type)
            .bind(resource_id)
            .bind(&context.source_ip)
            .bind(&context.session_family_id)
            .bind(&context.request_id)
            .bind(&context.user_agent)
            .bind(context.outcome.as_str())
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[derive(FromRow)]
struct UserRow {
    id: String,
    username: String,
    password_hash: String,
    role: String,
    is_active: bool,
    auth_version: i64,
}

impl UserRow {
    fn into_record(self) -> Result<UserRecord> {
        Ok(UserRecord {
            id: self.id,
            username: self.username,
            password_hash: self.password_hash,
            role: self.role.try_into()?,
            is_active: self.is_active,
            auth_version: self.auth_version,
        })
    }
}
