use chrono::Utc;
use serde_json::Value;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::{DatabaseError, Result};

const HISTORY_LENGTH: usize = 30;

#[derive(Debug, Clone)]
pub struct UptimeMonitorRecord {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub target: String,
    pub kind: String,
    pub interval_seconds: i64,
    pub enabled: bool,
    pub status: String,
    pub history: Vec<String>,
    pub latency_ms: Option<i64>,
    pub last_checked_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NewUptimeMonitor {
    pub user_id: String,
    pub name: String,
    pub target: String,
    pub kind: String,
    pub interval_seconds: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct UptimeMonitorUpdate {
    pub name: String,
    pub target: String,
    pub kind: String,
    pub interval_seconds: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct UptimeCheckUpdate {
    pub status: String,
    pub latency_ms: Option<u64>,
    pub last_error: Option<String>,
    pub checked_at: String,
}

#[derive(Debug, Clone)]
pub struct UptimeMonitorsRepository {
    pool: SqlitePool,
}

impl UptimeMonitorsRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_for_user(&self, user_id: &str) -> Result<Vec<UptimeMonitorRecord>> {
        let rows = sqlx::query_as::<_, UptimeMonitorRow>(
            "SELECT id, user_id, name, target, kind, interval_seconds, enabled, status,
                    history_json, latency_ms, last_checked_at, last_error, created_at, updated_at
             FROM uptime_monitors
             WHERE user_id = ?
             ORDER BY updated_at DESC, name COLLATE NOCASE",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(UptimeMonitorRow::into_record)
            .collect()
    }

    pub async fn list_enabled(&self) -> Result<Vec<UptimeMonitorRecord>> {
        let rows = sqlx::query_as::<_, UptimeMonitorRow>(
            "SELECT id, user_id, name, target, kind, interval_seconds, enabled, status,
                    history_json, latency_ms, last_checked_at, last_error, created_at, updated_at
             FROM uptime_monitors
             WHERE enabled = 1
             ORDER BY last_checked_at IS NOT NULL, last_checked_at",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(UptimeMonitorRow::into_record)
            .collect()
    }

    pub async fn create(&self, input: NewUptimeMonitor) -> Result<UptimeMonitorRecord> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let history = initial_history();
        let result = sqlx::query(
            "INSERT INTO uptime_monitors
             (id, user_id, name, target, kind, interval_seconds, enabled, status,
              history_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.user_id)
        .bind(&input.name)
        .bind(&input.target)
        .bind(&input.kind)
        .bind(i64::from(input.interval_seconds))
        .bind(input.enabled)
        .bind(
            serde_json::to_string(&history)
                .map_err(|error| sqlx::Error::Protocol(error.to_string()))?,
        )
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await;
        if let Err(error) = result {
            return map_name_conflict(error);
        }
        self.get_for_user(&input.user_id, &id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound.into())
    }

    pub async fn update(
        &self,
        user_id: &str,
        id: &str,
        input: UptimeMonitorUpdate,
    ) -> Result<Option<UptimeMonitorRecord>> {
        let now = Utc::now().to_rfc3339();
        let history = initial_history();
        let result = sqlx::query(
            "UPDATE uptime_monitors
             SET name = ?, target = ?, kind = ?, interval_seconds = ?, enabled = ?,
                 status = 'pending', history_json = ?, latency_ms = NULL,
                 last_checked_at = NULL, last_error = NULL, updated_at = ?
             WHERE id = ? AND user_id = ?",
        )
        .bind(&input.name)
        .bind(&input.target)
        .bind(&input.kind)
        .bind(i64::from(input.interval_seconds))
        .bind(input.enabled)
        .bind(
            serde_json::to_string(&history)
                .map_err(|error| sqlx::Error::Protocol(error.to_string()))?,
        )
        .bind(&now)
        .bind(id)
        .bind(user_id)
        .execute(&self.pool)
        .await;
        if let Err(error) = result {
            return map_name_conflict(error);
        }
        self.get_for_user(user_id, id).await
    }

    pub async fn delete(&self, user_id: &str, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM uptime_monitors WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn record_check(
        &self,
        id: &str,
        expected_updated_at: &str,
        update: UptimeCheckUpdate,
    ) -> Result<bool> {
        let existing = sqlx::query_as::<_, HistoryRow>(
            "SELECT history_json FROM uptime_monitors
             WHERE id = ? AND enabled = 1 AND updated_at = ?",
        )
        .bind(id)
        .bind(expected_updated_at)
        .fetch_optional(&self.pool)
        .await?;
        let Some(existing) = existing else {
            return Ok(false);
        };
        let mut history = parse_history(&existing.history_json)?;
        if history.len() >= HISTORY_LENGTH {
            history.remove(0);
        }
        history.push(update.status.clone());
        let result = sqlx::query(
            "UPDATE uptime_monitors
             SET status = ?, history_json = ?, latency_ms = ?, last_checked_at = ?,
                 last_error = ?, updated_at = ?
             WHERE id = ? AND enabled = 1 AND updated_at = ?",
        )
        .bind(&update.status)
        .bind(
            serde_json::to_string(&history)
                .map_err(|error| sqlx::Error::Protocol(error.to_string()))?,
        )
        .bind(
            update
                .latency_ms
                .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
        )
        .bind(&update.checked_at)
        .bind(&update.last_error)
        .bind(&update.checked_at)
        .bind(id)
        .bind(expected_updated_at)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    async fn get_for_user(&self, user_id: &str, id: &str) -> Result<Option<UptimeMonitorRecord>> {
        let row = sqlx::query_as::<_, UptimeMonitorRow>(
            "SELECT id, user_id, name, target, kind, interval_seconds, enabled, status,
                    history_json, latency_ms, last_checked_at, last_error, created_at, updated_at
             FROM uptime_monitors WHERE id = ? AND user_id = ?",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(UptimeMonitorRow::into_record).transpose()
    }
}

fn initial_history() -> Vec<String> {
    vec!["unknown".to_owned(); HISTORY_LENGTH]
}

fn parse_history(value: &str) -> Result<Vec<String>> {
    let parsed: Value = serde_json::from_str(value)
        .map_err(|error| DatabaseError::InvalidStoredUptimeHistory(error.to_string()))?;
    let history = parsed
        .as_array()
        .ok_or_else(|| DatabaseError::InvalidStoredUptimeHistory(value.to_owned()))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| DatabaseError::InvalidStoredUptimeHistory(value.to_owned()))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(history)
}

fn map_name_conflict<T>(error: sqlx::Error) -> Result<T> {
    match error {
        sqlx::Error::Database(database_error) if database_error.is_unique_violation() => {
            Err(DatabaseError::UptimeMonitorNameConflict)
        }
        error => Err(error.into()),
    }
}

#[derive(Debug, FromRow)]
struct UptimeMonitorRow {
    id: String,
    user_id: String,
    name: String,
    target: String,
    kind: String,
    interval_seconds: i64,
    enabled: i64,
    status: String,
    history_json: String,
    latency_ms: Option<i64>,
    last_checked_at: Option<String>,
    last_error: Option<String>,
    created_at: String,
    updated_at: String,
}

impl UptimeMonitorRow {
    fn into_record(self) -> Result<UptimeMonitorRecord> {
        Ok(UptimeMonitorRecord {
            id: self.id,
            user_id: self.user_id,
            name: self.name,
            target: self.target,
            kind: self.kind,
            interval_seconds: self.interval_seconds,
            enabled: self.enabled != 0,
            status: self.status,
            history: parse_history(&self.history_json)?,
            latency_ms: self.latency_ms,
            last_checked_at: self.last_checked_at,
            last_error: self.last_error,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

#[derive(Debug, FromRow)]
struct HistoryRow {
    history_json: String,
}
