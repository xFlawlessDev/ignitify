use chrono::Utc;
use serde_json::Value;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::{DatabaseError, Result};

#[derive(Debug, Clone)]
pub struct NotificationChannelRecord {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    pub event_types: Vec<String>,
    pub configuration_summary: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct NotificationChannelConnection {
    pub channel: NotificationChannelRecord,
    pub configuration_ciphertext: String,
}

#[derive(Debug, Clone)]
pub struct NewNotificationChannel {
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    pub event_types: Vec<String>,
    pub configuration_summary: Value,
    pub configuration_ciphertext: String,
}

#[derive(Debug, Clone)]
pub struct NotificationChannelsRepository {
    pool: SqlitePool,
}

impl NotificationChannelsRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<NotificationChannelRecord>> {
        let rows = sqlx::query_as::<_, NotificationChannelRow>(
            "SELECT id, name, kind, enabled, event_types_json, configuration_summary_json,
                    created_at, updated_at
             FROM notification_channels
             ORDER BY name COLLATE NOCASE, created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(NotificationChannelRow::into_record)
            .collect()
    }

    pub async fn enabled_for_event(
        &self,
        event_kind: &str,
    ) -> Result<Vec<NotificationChannelConnection>> {
        let rows = sqlx::query_as::<_, NotificationChannelConnectionRow>(
            "SELECT id, name, kind, enabled, event_types_json, configuration_summary_json,
                    configuration_ciphertext, created_at, updated_at
             FROM notification_channels
             WHERE enabled = 1",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(NotificationChannelConnectionRow::into_connection)
            .filter(|connection| {
                connection.as_ref().is_ok_and(|connection| {
                    connection
                        .channel
                        .event_types
                        .iter()
                        .any(|event| event == event_kind)
                })
            })
            .collect()
    }

    pub async fn create(&self, input: NewNotificationChannel) -> Result<NotificationChannelRecord> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "INSERT INTO notification_channels
             (id, name, kind, enabled, event_types_json, configuration_summary_json,
              configuration_ciphertext, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&input.kind)
        .bind(input.enabled)
        .bind(encode_event_types(&input.event_types)?)
        .bind(serde_json::to_string(&input.configuration_summary).map_err(invalid_channel)?)
        .bind(&input.configuration_ciphertext)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await;
        if let Err(error) = result {
            return map_name_conflict(error);
        }
        self.get(&id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound.into())
    }

    pub async fn update(
        &self,
        id: &str,
        input: NewNotificationChannel,
    ) -> Result<Option<NotificationChannelRecord>> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE notification_channels
             SET name = ?, kind = ?, enabled = ?, event_types_json = ?,
                 configuration_summary_json = ?, configuration_ciphertext = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&input.name)
        .bind(&input.kind)
        .bind(input.enabled)
        .bind(encode_event_types(&input.event_types)?)
        .bind(serde_json::to_string(&input.configuration_summary).map_err(invalid_channel)?)
        .bind(&input.configuration_ciphertext)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await;
        let result = match result {
            Ok(result) => result,
            Err(error) => return map_name_conflict(error),
        };
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.get(id).await
    }

    pub async fn delete(&self, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM notification_channels WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn claim_delivery(
        &self,
        channel_id: &str,
        source_kind: &str,
        source_id: &str,
        event_kind: &str,
    ) -> Result<bool> {
        let result = sqlx::query(
            "INSERT INTO notification_deliveries
             (id, channel_id, source_kind, source_id, event_kind, status, created_at)
             VALUES (?, ?, ?, ?, ?, 'running', ?)
             ON CONFLICT(channel_id, source_kind, source_id, event_kind) DO NOTHING",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(channel_id)
        .bind(source_kind)
        .bind(source_id)
        .bind(event_kind)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn finish_delivery(
        &self,
        channel_id: &str,
        source_kind: &str,
        source_id: &str,
        event_kind: &str,
        succeeded: bool,
    ) -> Result<()> {
        let status = if succeeded { "succeeded" } else { "failed" };
        let message = if succeeded {
            "Delivered".to_owned()
        } else {
            "Delivery failed; review the server logs".to_owned()
        };
        sqlx::query(
            "UPDATE notification_deliveries
             SET status = ?, completed_at = ?, message = ?
             WHERE channel_id = ? AND source_kind = ? AND source_id = ? AND event_kind = ?
               AND status = 'running'",
        )
        .bind(status)
        .bind(Utc::now().to_rfc3339())
        .bind(message)
        .bind(channel_id)
        .bind(source_kind)
        .bind(source_id)
        .bind(event_kind)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<NotificationChannelRecord>> {
        let row = sqlx::query_as::<_, NotificationChannelRow>(
            "SELECT id, name, kind, enabled, event_types_json, configuration_summary_json,
                    created_at, updated_at
             FROM notification_channels WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(NotificationChannelRow::into_record).transpose()
    }
}

#[derive(Debug, FromRow)]
struct NotificationChannelRow {
    id: String,
    name: String,
    kind: String,
    enabled: bool,
    event_types_json: String,
    configuration_summary_json: String,
    created_at: String,
    updated_at: String,
}

impl NotificationChannelRow {
    fn into_record(self) -> Result<NotificationChannelRecord> {
        Ok(NotificationChannelRecord {
            id: self.id,
            name: self.name,
            kind: self.kind,
            enabled: self.enabled,
            event_types: decode_event_types(&self.event_types_json)?,
            configuration_summary: serde_json::from_str(&self.configuration_summary_json)
                .map_err(invalid_channel)?,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

#[derive(Debug, FromRow)]
struct NotificationChannelConnectionRow {
    id: String,
    name: String,
    kind: String,
    enabled: bool,
    event_types_json: String,
    configuration_summary_json: String,
    configuration_ciphertext: String,
    created_at: String,
    updated_at: String,
}

impl NotificationChannelConnectionRow {
    fn into_connection(self) -> Result<NotificationChannelConnection> {
        Ok(NotificationChannelConnection {
            channel: NotificationChannelRow {
                id: self.id,
                name: self.name,
                kind: self.kind,
                enabled: self.enabled,
                event_types_json: self.event_types_json,
                configuration_summary_json: self.configuration_summary_json,
                created_at: self.created_at,
                updated_at: self.updated_at,
            }
            .into_record()?,
            configuration_ciphertext: self.configuration_ciphertext,
        })
    }
}

fn encode_event_types(events: &[String]) -> Result<String> {
    serde_json::to_string(events).map_err(invalid_channel)
}

fn decode_event_types(value: &str) -> Result<Vec<String>> {
    serde_json::from_str(value).map_err(invalid_channel)
}

fn invalid_channel(error: impl std::fmt::Display) -> DatabaseError {
    DatabaseError::InvalidStoredNotificationChannel(error.to_string())
}

fn map_name_conflict<T>(error: sqlx::Error) -> Result<T> {
    match error {
        sqlx::Error::Database(database_error) if database_error.is_unique_violation() => {
            Err(DatabaseError::NotificationChannelNameConflict)
        }
        error => Err(error.into()),
    }
}
