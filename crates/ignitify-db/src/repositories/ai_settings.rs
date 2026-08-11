use chrono::Utc;
use sqlx::{FromRow, SqlitePool};

use crate::Result;

#[derive(Debug, Clone)]
pub struct AiSettingsRecord {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub api_key_configured: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct AiSettingsConnection {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub api_key_ciphertext: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewAiSettings {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub api_key_ciphertext: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AiSettingsRepository {
    pool: SqlitePool,
}

impl AiSettingsRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get(&self) -> Result<AiSettingsRecord> {
        let row = sqlx::query_as::<_, AiSettingsRow>(
            "SELECT enabled, base_url, model, api_key_ciphertext, created_at, updated_at
             FROM ai_settings WHERE id = 1",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into_record())
    }

    pub async fn connection(&self) -> Result<AiSettingsConnection> {
        let row = sqlx::query_as::<_, AiSettingsConnectionRow>(
            "SELECT enabled, base_url, model, api_key_ciphertext
             FROM ai_settings WHERE id = 1",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.into_connection())
    }

    pub async fn upsert(&self, input: NewAiSettings) -> Result<AiSettingsRecord> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE ai_settings
             SET enabled = ?, base_url = ?, model = ?, api_key_ciphertext = ?, updated_at = ?
             WHERE id = 1",
        )
        .bind(input.enabled)
        .bind(input.base_url)
        .bind(input.model)
        .bind(input.api_key_ciphertext)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.get().await
    }
}

#[derive(Debug, FromRow)]
struct AiSettingsRow {
    enabled: bool,
    base_url: String,
    model: String,
    api_key_ciphertext: Option<String>,
    created_at: String,
    updated_at: String,
}

impl AiSettingsRow {
    fn into_record(self) -> AiSettingsRecord {
        AiSettingsRecord {
            enabled: self.enabled,
            base_url: self.base_url,
            model: self.model,
            api_key_configured: self.api_key_ciphertext.is_some(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct AiSettingsConnectionRow {
    enabled: bool,
    base_url: String,
    model: String,
    api_key_ciphertext: Option<String>,
}

impl AiSettingsConnectionRow {
    fn into_connection(self) -> AiSettingsConnection {
        AiSettingsConnection {
            enabled: self.enabled,
            base_url: self.base_url,
            model: self.model,
            api_key_ciphertext: self.api_key_ciphertext,
        }
    }
}
