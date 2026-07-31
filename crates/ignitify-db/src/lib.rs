//! SQLite persistence for Ignitify.

use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use sqlx::{FromRow, SqlitePool, sqlite::SqliteConnectOptions};
use thiserror::Error;
use uuid::Uuid;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("invalid user role: {0}")]
    InvalidRole(String),
}

pub type Result<T> = std::result::Result<T, DatabaseError>;

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite:data/ignitify.db".to_owned(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn connect(config: &DatabaseConfig) -> Result<Self> {
        let path = sqlite_file_path(&config.url);
        if let Some(path) = &path
            && let Some(parent) = path.parent()
        {
            std::fs::create_dir_all(parent)?;
        }

        let is_memory = path.is_none();
        let options = SqliteConnectOptions::new()
            .filename(path.unwrap_or_else(|| PathBuf::from(":memory:")))
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = if is_memory {
            sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await?
        } else {
            SqlitePool::connect_with(options).await?
        };
        MIGRATOR.run(&pool).await?;
        Ok(Self { pool })
    }

    pub fn users(&self) -> UsersRepository {
        UsersRepository::new(self.pool.clone())
    }

    pub fn refresh_tokens(&self) -> RefreshTokensRepository {
        RefreshTokensRepository::new(self.pool.clone())
    }

    pub async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }
}

fn sqlite_file_path(url: &str) -> Option<PathBuf> {
    let path = url
        .strip_prefix("sqlite://")
        .or_else(|| url.strip_prefix("sqlite:"))?;
    if path == ":memory:" {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserRole {
    Admin,
    User,
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::User => "user",
        }
    }
}

impl TryFrom<String> for UserRole {
    type Error = DatabaseError;

    fn try_from(value: String) -> Result<Self> {
        match value.as_str() {
            "admin" => Ok(Self::Admin),
            "user" => Ok(Self::User),
            _ => Err(DatabaseError::InvalidRole(value)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UserRecord {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub role: UserRole,
    pub is_active: bool,
    pub auth_version: i64,
}

#[derive(Debug, Clone)]
pub struct UsersRepository {
    pool: SqlitePool,
}

impl UsersRepository {
    fn new(pool: SqlitePool) -> Self {
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
        sqlx::query("INSERT INTO audit_logs (id, user_id, action, created_at) VALUES (?, ?, ?, ?)")
            .bind(Uuid::new_v4().to_string())
            .bind(user_id)
            .bind(action)
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

#[derive(Debug, Clone)]
pub struct RefreshTokenRecord {
    pub user_id: String,
    pub family_id: String,
    pub expires_at: DateTime<Utc>,
    pub absolute_expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum RotateRefreshTokenOutcome {
    Rotated(RefreshTokenRecord),
    Missing,
    Expired,
    Reused { user_id: String, family_id: String },
}

#[derive(Debug, Clone)]
pub struct RefreshTokensRepository {
    pool: SqlitePool,
}

impl RefreshTokensRepository {
    fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        user_id: &str,
        token_hash: &str,
        idle_ttl: Duration,
        absolute_ttl: Duration,
    ) -> Result<RefreshTokenRecord> {
        let now = Utc::now();
        let family_id = Uuid::new_v4().to_string();
        let absolute_expires_at = now + absolute_ttl;
        let expires_at = (now + idle_ttl).min(absolute_expires_at);
        self.insert(
            user_id,
            token_hash,
            &family_id,
            now,
            expires_at,
            absolute_expires_at,
        )
        .await
    }

    pub async fn rotate(
        &self,
        token_hash: &str,
        successor_hash: &str,
        idle_ttl: Duration,
    ) -> Result<RotateRefreshTokenOutcome> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query_as::<_, RefreshTokenRow>(
            "SELECT user_id, family_id, expires_at, absolute_expires_at, revoked_at FROM refresh_tokens WHERE token_hash = ?",
        )
        .bind(token_hash)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            tx.rollback().await?;
            return Ok(RotateRefreshTokenOutcome::Missing);
        };
        let (record, revoked) = row.into_record()?;
        if revoked {
            tx.rollback().await?;
            return Ok(RotateRefreshTokenOutcome::Reused {
                user_id: record.user_id,
                family_id: record.family_id,
            });
        }
        let now = Utc::now();
        if record.expires_at <= now || record.absolute_expires_at <= now {
            tx.rollback().await?;
            return Ok(RotateRefreshTokenOutcome::Expired);
        }
        let consumed = sqlx::query(
            "UPDATE refresh_tokens SET revoked_at = ?, replaced_by = ? WHERE token_hash = ? AND revoked_at IS NULL",
        )
        .bind(now.to_rfc3339())
        .bind(successor_hash)
        .bind(token_hash)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if consumed != 1 {
            tx.rollback().await?;
            return Ok(RotateRefreshTokenOutcome::Reused {
                user_id: record.user_id,
                family_id: record.family_id,
            });
        }
        let expires_at = (now + idle_ttl).min(record.absolute_expires_at);
        let successor = Self::insert_in_transaction(
            &mut tx,
            &record.user_id,
            successor_hash,
            &record.family_id,
            now,
            expires_at,
            record.absolute_expires_at,
        )
        .await?;
        tx.commit().await?;
        Ok(RotateRefreshTokenOutcome::Rotated(successor))
    }

    pub async fn revoke_family(&self, user_id: &str, family_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE refresh_tokens SET revoked_at = ? WHERE user_id = ? AND family_id = ? AND revoked_at IS NULL",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(user_id)
        .bind(family_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn has_live_family(&self, user_id: &str, family_id: &str) -> Result<bool> {
        let now = Utc::now().to_rfc3339();
        Ok(sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM refresh_tokens WHERE user_id = ? AND family_id = ? AND revoked_at IS NULL AND expires_at > ? AND absolute_expires_at > ?)",
        )
        .bind(user_id)
        .bind(family_id)
        .bind(&now)
        .bind(&now)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn revoke_family_by_hash(&self, token_hash: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE refresh_tokens SET revoked_at = ? WHERE family_id = (SELECT family_id FROM refresh_tokens WHERE token_hash = ?) AND revoked_at IS NULL",
        )
        .bind(now)
        .bind(token_hash)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn insert(
        &self,
        user_id: &str,
        token_hash: &str,
        family_id: &str,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        absolute_expires_at: DateTime<Utc>,
    ) -> Result<RefreshTokenRecord> {
        sqlx::query(
            "INSERT INTO refresh_tokens (id, user_id, token_hash, family_id, created_at, expires_at, absolute_expires_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(token_hash)
        .bind(family_id)
        .bind(created_at.to_rfc3339())
        .bind(expires_at.to_rfc3339())
        .bind(absolute_expires_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(RefreshTokenRecord {
            user_id: user_id.to_owned(),
            family_id: family_id.to_owned(),
            expires_at,
            absolute_expires_at,
        })
    }

    async fn insert_in_transaction(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        user_id: &str,
        token_hash: &str,
        family_id: &str,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        absolute_expires_at: DateTime<Utc>,
    ) -> Result<RefreshTokenRecord> {
        sqlx::query(
            "INSERT INTO refresh_tokens (id, user_id, token_hash, family_id, created_at, expires_at, absolute_expires_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(token_hash)
        .bind(family_id)
        .bind(created_at.to_rfc3339())
        .bind(expires_at.to_rfc3339())
        .bind(absolute_expires_at.to_rfc3339())
        .execute(&mut **tx)
        .await?;
        Ok(RefreshTokenRecord {
            user_id: user_id.to_owned(),
            family_id: family_id.to_owned(),
            expires_at,
            absolute_expires_at,
        })
    }
}

#[derive(FromRow)]
struct RefreshTokenRow {
    user_id: String,
    family_id: String,
    expires_at: String,
    absolute_expires_at: String,
    revoked_at: Option<String>,
}

impl RefreshTokenRow {
    fn into_record(self) -> Result<(RefreshTokenRecord, bool)> {
        Ok((
            RefreshTokenRecord {
                user_id: self.user_id,
                family_id: self.family_id,
                expires_at: DateTime::parse_from_rfc3339(&self.expires_at)
                    .map_err(|error| sqlx::Error::Decode(Box::new(error)))?
                    .with_timezone(&Utc),
                absolute_expires_at: DateTime::parse_from_rfc3339(&self.absolute_expires_at)
                    .map_err(|error| sqlx::Error::Decode(Box::new(error)))?
                    .with_timezone(&Utc),
            },
            self.revoked_at.is_some(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{Database, DatabaseConfig};

    #[tokio::test]
    async fn migrations_create_auth_storage() {
        let database = Database::connect(&DatabaseConfig {
            url: "sqlite::memory:".to_owned(),
        })
        .await
        .unwrap();

        assert_eq!(database.users().count().await.unwrap(), 0);
    }
}
