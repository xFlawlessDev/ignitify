use chrono::Utc;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::Result;

#[derive(Debug, Clone)]
pub struct RemoteServerAgentRecord {
    pub server_id: String,
    pub status: String,
    pub version: Option<String>,
    pub cpu_usage_percentage: Option<f64>,
    pub cpu_cores: Option<i64>,
    pub memory_used_bytes: Option<i64>,
    pub memory_total_bytes: Option<i64>,
    pub disk_used_bytes: Option<i64>,
    pub disk_total_bytes: Option<i64>,
    pub docker_containers: Option<i64>,
    pub docker_running_containers: Option<i64>,
    pub last_heartbeat_at: Option<String>,
    pub last_error: Option<String>,
    pub installed_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct RemoteServerAgentHeartbeat {
    pub version: String,
    pub cpu_usage_percentage: Option<f64>,
    pub cpu_cores: Option<i64>,
    pub memory_used_bytes: Option<i64>,
    pub memory_total_bytes: Option<i64>,
    pub disk_used_bytes: Option<i64>,
    pub disk_total_bytes: Option<i64>,
    pub docker_containers: Option<i64>,
    pub docker_running_containers: Option<i64>,
    pub reported_at: String,
}

#[derive(Debug, Clone)]
pub struct RemoteServerAgentsRepository {
    pool: SqlitePool,
}

impl RemoteServerAgentsRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, server_id: &str) -> Result<Option<RemoteServerAgentRecord>> {
        let row = sqlx::query_as::<_, RemoteServerAgentRow>(
            "SELECT server_id, status, version, cpu_usage_percentage, cpu_cores,
                    memory_used_bytes, memory_total_bytes, disk_used_bytes, disk_total_bytes,
                    docker_containers, docker_running_containers, last_heartbeat_at,
                    last_error, installed_at, updated_at
             FROM remote_server_agents
             WHERE server_id = ?",
        )
        .bind(server_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(RemoteServerAgentRow::into_record))
    }

    pub async fn token_hash(&self, server_id: &str) -> Result<Option<String>> {
        Ok(sqlx::query_scalar::<_, String>(
            "SELECT token_hash FROM remote_server_agents WHERE server_id = ?",
        )
        .bind(server_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn install(
        &self,
        server_id: &str,
        token_hash: &str,
    ) -> Result<RemoteServerAgentRecord> {
        let mut transaction = self.pool.begin().await?;
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM remote_servers WHERE id = ?)",
        )
        .bind(server_id)
        .fetch_one(&mut *transaction)
        .await?;
        if exists == 0 {
            transaction.rollback().await?;
            return Err(sqlx::Error::RowNotFound.into());
        }
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO remote_server_agents
                (server_id, token_hash, status, installed_at, updated_at)
             VALUES (?, ?, 'pending', ?, ?)
             ON CONFLICT(server_id) DO UPDATE SET
                token_hash = excluded.token_hash,
                status = 'pending',
                version = NULL,
                cpu_usage_percentage = NULL,
                cpu_cores = NULL,
                memory_used_bytes = NULL,
                memory_total_bytes = NULL,
                disk_used_bytes = NULL,
                disk_total_bytes = NULL,
                docker_containers = NULL,
                docker_running_containers = NULL,
                last_heartbeat_at = NULL,
                last_error = NULL,
                installed_at = excluded.installed_at,
                updated_at = excluded.updated_at",
        )
        .bind(server_id)
        .bind(token_hash)
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;
        insert_event(
            &mut transaction,
            server_id,
            "provisioned",
            "agent provisioning requested",
            &now,
        )
        .await?;
        transaction.commit().await?;
        self.get(server_id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound.into())
    }

    pub async fn record_heartbeat(
        &self,
        server_id: &str,
        heartbeat: &RemoteServerAgentHeartbeat,
    ) -> Result<Option<RemoteServerAgentRecord>> {
        let mut transaction = self.pool.begin().await?;
        let previous_status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM remote_server_agents WHERE server_id = ?",
        )
        .bind(server_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(previous_status) = previous_status else {
            transaction.rollback().await?;
            return Ok(None);
        };
        let updated_at = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE remote_server_agents
             SET status = 'online', version = ?, cpu_usage_percentage = ?, cpu_cores = ?,
                 memory_used_bytes = ?, memory_total_bytes = ?, disk_used_bytes = ?,
                 disk_total_bytes = ?, docker_containers = ?, docker_running_containers = ?,
                 last_heartbeat_at = ?, last_error = NULL, updated_at = ?
             WHERE server_id = ?",
        )
        .bind(&heartbeat.version)
        .bind(heartbeat.cpu_usage_percentage)
        .bind(heartbeat.cpu_cores)
        .bind(heartbeat.memory_used_bytes)
        .bind(heartbeat.memory_total_bytes)
        .bind(heartbeat.disk_used_bytes)
        .bind(heartbeat.disk_total_bytes)
        .bind(heartbeat.docker_containers)
        .bind(heartbeat.docker_running_containers)
        .bind(&heartbeat.reported_at)
        .bind(&updated_at)
        .bind(server_id)
        .execute(&mut *transaction)
        .await?;
        if previous_status != "online" {
            insert_event(
                &mut transaction,
                server_id,
                "online",
                "agent heartbeat received",
                &updated_at,
            )
            .await?;
        }
        transaction.commit().await?;
        self.get(server_id).await
    }

    pub async fn record_error(&self, server_id: &str, message: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query(
            "UPDATE remote_server_agents
             SET status = 'offline', last_error = ?, updated_at = ?
             WHERE server_id = ?",
        )
        .bind(message)
        .bind(&now)
        .bind(server_id)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() == 0 {
            transaction.rollback().await?;
            return Ok(());
        }
        insert_event(&mut transaction, server_id, "error", message, &now).await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn mark_stale(&self, cutoff: &str) -> Result<()> {
        let mut transaction = self.pool.begin().await?;
        let rows = sqlx::query_as::<_, StaleAgentRow>(
            "SELECT server_id FROM remote_server_agents
             WHERE status = 'online' AND (last_heartbeat_at IS NULL OR last_heartbeat_at < ?)",
        )
        .bind(cutoff)
        .fetch_all(&mut *transaction)
        .await?;
        let now = Utc::now().to_rfc3339();
        for row in rows {
            sqlx::query(
                "UPDATE remote_server_agents
                 SET status = 'offline', last_error = 'agent heartbeat timed out', updated_at = ?
                 WHERE server_id = ?",
            )
            .bind(&now)
            .bind(&row.server_id)
            .execute(&mut *transaction)
            .await?;
            insert_event(
                &mut transaction,
                &row.server_id,
                "offline",
                "agent heartbeat timed out",
                &now,
            )
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }
}

async fn insert_event(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    server_id: &str,
    kind: &str,
    message: &str,
    created_at: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO remote_server_agent_events (id, server_id, kind, message, created_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(server_id)
    .bind(kind)
    .bind(message)
    .bind(created_at)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

#[derive(Debug, FromRow)]
struct RemoteServerAgentRow {
    server_id: String,
    status: String,
    version: Option<String>,
    cpu_usage_percentage: Option<f64>,
    cpu_cores: Option<i64>,
    memory_used_bytes: Option<i64>,
    memory_total_bytes: Option<i64>,
    disk_used_bytes: Option<i64>,
    disk_total_bytes: Option<i64>,
    docker_containers: Option<i64>,
    docker_running_containers: Option<i64>,
    last_heartbeat_at: Option<String>,
    last_error: Option<String>,
    installed_at: String,
    updated_at: String,
}

impl RemoteServerAgentRow {
    fn into_record(self) -> RemoteServerAgentRecord {
        RemoteServerAgentRecord {
            server_id: self.server_id,
            status: self.status,
            version: self.version,
            cpu_usage_percentage: self.cpu_usage_percentage,
            cpu_cores: self.cpu_cores,
            memory_used_bytes: self.memory_used_bytes,
            memory_total_bytes: self.memory_total_bytes,
            disk_used_bytes: self.disk_used_bytes,
            disk_total_bytes: self.disk_total_bytes,
            docker_containers: self.docker_containers,
            docker_running_containers: self.docker_running_containers,
            last_heartbeat_at: self.last_heartbeat_at,
            last_error: self.last_error,
            installed_at: self.installed_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct StaleAgentRow {
    server_id: String,
}
