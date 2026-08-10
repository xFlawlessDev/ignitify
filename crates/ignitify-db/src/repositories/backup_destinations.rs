use chrono::Utc;
use sqlx::{FromRow, SqlitePool};

use crate::Result;

#[derive(Debug, Clone)]
pub struct BackupS3DestinationRecord {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub prefix: String,
    pub server_side_encryption: String,
    pub enabled: bool,
    pub schedule_interval_hours: Option<u16>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct BackupS3RunRecord {
    pub id: String,
    pub trigger: String,
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BackupS3DestinationConnection {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub prefix: String,
    pub access_key_id_ciphertext: String,
    pub secret_access_key_ciphertext: String,
    pub session_token_ciphertext: Option<String>,
    pub server_side_encryption: String,
}

#[derive(Debug, Clone)]
pub struct NewBackupS3Destination {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub prefix: String,
    pub access_key_id_ciphertext: String,
    pub secret_access_key_ciphertext: String,
    pub session_token_ciphertext: Option<String>,
    pub server_side_encryption: String,
}

#[derive(Debug, Clone)]
pub struct BackupDestinationsRepository {
    pool: SqlitePool,
}

impl BackupDestinationsRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn s3(&self) -> Result<Option<BackupS3DestinationRecord>> {
        let row = sqlx::query_as::<_, BackupS3DestinationRow>(
            "SELECT endpoint, region, bucket, prefix, server_side_encryption, enabled,
                    schedule_interval_hours, created_at, updated_at
             FROM backup_s3_destination WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(BackupS3DestinationRow::into_record))
    }

    pub async fn s3_connection(&self) -> Result<Option<BackupS3DestinationConnection>> {
        let row = sqlx::query_as::<_, BackupS3DestinationConnectionRow>(
            "SELECT endpoint, region, bucket, prefix,
                    access_key_id_ciphertext, secret_access_key_ciphertext,
                    session_token_ciphertext, server_side_encryption
             FROM backup_s3_destination WHERE id = 1 AND enabled = 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(BackupS3DestinationConnectionRow::into_connection))
    }

    pub async fn upsert_s3(
        &self,
        input: NewBackupS3Destination,
    ) -> Result<BackupS3DestinationRecord> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO backup_s3_destination
             (id, endpoint, region, bucket, prefix, access_key_id_ciphertext,
              secret_access_key_ciphertext, session_token_ciphertext, server_side_encryption,
              created_at, updated_at)
             VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                endpoint = excluded.endpoint,
                region = excluded.region,
                bucket = excluded.bucket,
                prefix = excluded.prefix,
                access_key_id_ciphertext = excluded.access_key_id_ciphertext,
                secret_access_key_ciphertext = excluded.secret_access_key_ciphertext,
                session_token_ciphertext = excluded.session_token_ciphertext,
                server_side_encryption = excluded.server_side_encryption,
                updated_at = excluded.updated_at",
        )
        .bind(&input.endpoint)
        .bind(&input.region)
        .bind(&input.bucket)
        .bind(&input.prefix)
        .bind(&input.access_key_id_ciphertext)
        .bind(&input.secret_access_key_ciphertext)
        .bind(&input.session_token_ciphertext)
        .bind(&input.server_side_encryption)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.s3()
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound.into())
    }

    pub async fn update_s3_controls(
        &self,
        enabled: bool,
        schedule_interval_hours: Option<u16>,
    ) -> Result<Option<BackupS3DestinationRecord>> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE backup_s3_destination
             SET enabled = ?, schedule_interval_hours = ?, updated_at = ?
             WHERE id = 1",
        )
        .bind(enabled)
        .bind(schedule_interval_hours)
        .bind(now)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.s3().await
    }

    pub async fn list_s3_runs(&self, limit: u16) -> Result<Vec<BackupS3RunRecord>> {
        let rows = sqlx::query_as::<_, BackupS3RunRow>(
            "SELECT id, trigger, status, started_at, completed_at, message
             FROM backup_s3_run
             ORDER BY started_at DESC
             LIMIT ?",
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(BackupS3RunRow::into_record).collect())
    }

    pub async fn latest_scheduled_s3_run(&self) -> Result<Option<BackupS3RunRecord>> {
        let row = sqlx::query_as::<_, BackupS3RunRow>(
            "SELECT id, trigger, status, started_at, completed_at, message
             FROM backup_s3_run
             WHERE trigger = 'scheduled'
             ORDER BY started_at DESC
             LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(BackupS3RunRow::into_record))
    }

    pub async fn start_s3_run(&self, id: &str, trigger: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO backup_s3_run (id, trigger, status, started_at)
             VALUES (?, ?, 'running', ?)",
        )
        .bind(id)
        .bind(trigger)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn finish_s3_run(&self, id: &str, succeeded: bool) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let (status, message) = if succeeded {
            ("succeeded", "Backup completed")
        } else {
            ("failed", "Backup failed; review the server logs")
        };
        sqlx::query(
            "UPDATE backup_s3_run
             SET status = ?, completed_at = ?, message = ?
             WHERE id = ? AND status = 'running'",
        )
        .bind(status)
        .bind(now)
        .bind(message)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_s3(&self) -> Result<bool> {
        let result = sqlx::query("DELETE FROM backup_s3_destination WHERE id = 1")
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
    }
}

#[derive(Debug, FromRow)]
struct BackupS3DestinationRow {
    endpoint: String,
    region: String,
    bucket: String,
    prefix: String,
    server_side_encryption: String,
    enabled: bool,
    schedule_interval_hours: Option<u16>,
    created_at: String,
    updated_at: String,
}

impl BackupS3DestinationRow {
    fn into_record(self) -> BackupS3DestinationRecord {
        BackupS3DestinationRecord {
            endpoint: self.endpoint,
            region: self.region,
            bucket: self.bucket,
            prefix: self.prefix,
            server_side_encryption: self.server_side_encryption,
            enabled: self.enabled,
            schedule_interval_hours: self.schedule_interval_hours,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct BackupS3RunRow {
    id: String,
    trigger: String,
    status: String,
    started_at: String,
    completed_at: Option<String>,
    message: Option<String>,
}

impl BackupS3RunRow {
    fn into_record(self) -> BackupS3RunRecord {
        BackupS3RunRecord {
            id: self.id,
            trigger: self.trigger,
            status: self.status,
            started_at: self.started_at,
            completed_at: self.completed_at,
            message: self.message,
        }
    }
}

#[derive(Debug, FromRow)]
struct BackupS3DestinationConnectionRow {
    endpoint: String,
    region: String,
    bucket: String,
    prefix: String,
    access_key_id_ciphertext: String,
    secret_access_key_ciphertext: String,
    session_token_ciphertext: Option<String>,
    server_side_encryption: String,
}

impl BackupS3DestinationConnectionRow {
    fn into_connection(self) -> BackupS3DestinationConnection {
        BackupS3DestinationConnection {
            endpoint: self.endpoint,
            region: self.region,
            bucket: self.bucket,
            prefix: self.prefix,
            access_key_id_ciphertext: self.access_key_id_ciphertext,
            secret_access_key_ciphertext: self.secret_access_key_ciphertext,
            session_token_ciphertext: self.session_token_ciphertext,
            server_side_encryption: self.server_side_encryption,
        }
    }
}
