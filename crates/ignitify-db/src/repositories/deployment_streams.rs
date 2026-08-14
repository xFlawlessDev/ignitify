use chrono::Utc;
use sqlx::{FromRow, SqlitePool};

use super::{
    DeploymentEventRecord, DeploymentId, DeploymentLogRecord, DeploymentsRepository,
    NewDeploymentLog, Result, SequenceCursor,
};

impl DeploymentsRepository {
    pub async fn latest_log_since(&self, deployment_id: &str) -> Result<Option<i64>> {
        Ok(sqlx::query_scalar(
            "SELECT CAST(strftime('%s', MAX(created_at)) AS INTEGER)\n             FROM deployment_logs WHERE deployment_id = ?",
        )
        .bind(deployment_id)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn append_logs(
        &self,
        deployment_id: &str,
        logs: &[NewDeploymentLog],
    ) -> Result<Vec<DeploymentLogRecord>> {
        if logs.is_empty() {
            return Ok(Vec::new());
        }
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let correlation_id: String =
            sqlx::query_scalar("SELECT correlation_id FROM deployments WHERE id = ?")
                .bind(deployment_id)
                .fetch_one(&mut *tx)
                .await?;
        let mut inserted = Vec::with_capacity(logs.len());
        for log in logs {
            let line = bound_log_line(&log.line);
            let sequence: i64 = sqlx::query_scalar(
                "INSERT INTO deployment_logs (deployment_id, correlation_id, stream, line, created_at)\n                 VALUES (?, ?, ?, ?, ?) RETURNING sequence",
            )
            .bind(deployment_id)
            .bind(&correlation_id)
            .bind(&log.stream)
            .bind(&line)
            .bind(&now)
            .fetch_one(&mut *tx)
            .await?;
            inserted.push(DeploymentLogRecord {
                sequence,
                deployment_id: DeploymentId::new(deployment_id)
                    .map_err(|_| sqlx::Error::Protocol("stored deployment id is invalid".into()))?,
                correlation_id: correlation_id.clone(),
                stream: log.stream.clone(),
                line,
                created_at: now.clone(),
            });
        }
        sqlx::query(
            "DELETE FROM deployment_logs\n             WHERE deployment_id = ? AND sequence <= COALESCE(\n                (SELECT sequence FROM deployment_logs\n                 WHERE deployment_id = ?\n                 ORDER BY sequence DESC LIMIT 1 OFFSET 10000),\n                -1\n             )",
        )
        .bind(deployment_id)
        .bind(deployment_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(inserted)
    }

    pub async fn prune_retention(&self) -> Result<()> {
        let cutoff = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM deployment_events\n             WHERE deployment_id IN (\n                SELECT id FROM deployments\n                WHERE status IN ('healthy', 'failed', 'stopped', 'superseded')\n                  AND finished_at < ?\n             )",
        )
        .bind(&cutoff)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "DELETE FROM deployment_logs\n             WHERE deployment_id IN (\n                SELECT id FROM deployments\n                WHERE status IN ('healthy', 'failed', 'stopped', 'superseded')\n                  AND finished_at < ?\n             )",
        )
        .bind(cutoff)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn events(&self, deployment_id: &str) -> Result<Vec<DeploymentEventRecord>> {
        self.events_after(deployment_id, 0, i64::MAX).await
    }

    pub async fn events_after(
        &self,
        deployment_id: &str,
        after: i64,
        through: i64,
    ) -> Result<Vec<DeploymentEventRecord>> {
        let rows = sqlx::query_as::<_, EventRow>(
            "SELECT sequence, deployment_id, correlation_id, kind, payload_json, created_at\n             FROM deployment_events\n             WHERE deployment_id = ? AND sequence > ? AND sequence <= ?\n             ORDER BY sequence",
        )
        .bind(deployment_id)
        .bind(after)
        .bind(through)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(DeploymentEventRecord {
                    sequence: row.sequence,
                    event_id: event_id(&row.deployment_id, row.sequence),
                    deployment_id: DeploymentId::new(row.deployment_id).map_err(|_| {
                        sqlx::Error::Protocol("stored deployment id is invalid".into())
                    })?,
                    correlation_id: row.correlation_id,
                    kind: row.kind,
                    payload_json: row.payload_json,
                    created_at: row.created_at,
                })
            })
            .collect()
    }

    pub async fn event_cursor(&self, deployment_id: &str) -> Result<SequenceCursor> {
        cursor(&self.pool, "deployment_events", deployment_id).await
    }

    pub async fn logs_after(
        &self,
        deployment_id: &str,
        after: i64,
        through: i64,
    ) -> Result<Vec<DeploymentLogRecord>> {
        let rows = sqlx::query_as::<_, LogRow>(
            "SELECT sequence, deployment_id, correlation_id, stream, line, created_at\n             FROM deployment_logs\n             WHERE deployment_id = ? AND sequence > ? AND sequence <= ?\n             ORDER BY sequence",
        )
        .bind(deployment_id)
        .bind(after)
        .bind(through)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(DeploymentLogRecord {
                    sequence: row.sequence,
                    deployment_id: DeploymentId::new(row.deployment_id).map_err(|_| {
                        sqlx::Error::Protocol("stored deployment id is invalid".into())
                    })?,
                    correlation_id: row.correlation_id,
                    stream: row.stream,
                    line: row.line,
                    created_at: row.created_at,
                })
            })
            .collect()
    }

    pub async fn log_cursor(&self, deployment_id: &str) -> Result<SequenceCursor> {
        cursor(&self.pool, "deployment_logs", deployment_id).await
    }
}

fn bound_log_line(line: &str) -> String {
    let mut end = line.len().min(16 * 1024);
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    line[..end].to_owned()
}

fn event_id(deployment_id: &str, sequence: i64) -> String {
    format!("deployment/{deployment_id}/event/{sequence}")
}

async fn cursor(pool: &SqlitePool, table: &str, deployment_id: &str) -> Result<SequenceCursor> {
    let query = format!("SELECT MIN(sequence), MAX(sequence) FROM {table} WHERE deployment_id = ?");
    let (oldest, newest): (Option<i64>, Option<i64>) = sqlx::query_as(&query)
        .bind(deployment_id)
        .fetch_one(pool)
        .await?;
    Ok(SequenceCursor { oldest, newest })
}

#[derive(Debug, FromRow)]
struct EventRow {
    sequence: i64,
    deployment_id: String,
    correlation_id: String,
    kind: String,
    payload_json: String,
    created_at: String,
}

#[derive(Debug, FromRow)]
struct LogRow {
    sequence: i64,
    deployment_id: String,
    correlation_id: String,
    stream: String,
    line: String,
    created_at: String,
}
