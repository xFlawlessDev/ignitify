use chrono::{DateTime, Duration, Utc};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::{RefreshTokenRecord, Result, RotateRefreshTokenOutcome};

#[derive(Debug, Clone)]
pub struct RefreshTokensRepository {
    pool: SqlitePool,
}

impl RefreshTokensRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
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
