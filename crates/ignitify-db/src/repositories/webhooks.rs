use chrono::Utc;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::{DatabaseError, Result};

#[derive(Debug, Clone, Copy)]
pub struct WebhookActor<'a> {
    pub id: &'a str,
    pub is_admin: bool,
}

#[derive(Debug, Clone)]
pub struct NewWebhook {
    pub name: String,
    pub url: String,
    pub secret_ciphertext: Option<String>,
    pub is_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct WebhookRecord {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub url: String,
    pub secret_ciphertext: Option<String>,
    pub is_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub enum WebhookMutationOutcome {
    Created(WebhookRecord),
    Removed(WebhookRecord),
    Missing,
    Forbidden,
}

#[derive(Debug, Clone)]
pub struct WebhooksRepository {
    pool: SqlitePool,
}

impl WebhooksRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(
        &self,
        actor: WebhookActor<'_>,
        project_id: &str,
    ) -> Result<Option<Vec<WebhookRecord>>> {
        if self.project_role(actor, project_id).await?.is_none() {
            return Ok(None);
        }
        let records = sqlx::query_as::<_, WebhookRow>(
            "SELECT id, project_id, name, url, secret_ciphertext, is_enabled, created_at, updated_at
             FROM project_webhooks WHERE project_id = ? ORDER BY updated_at DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(WebhookRecord::from)
        .collect();
        Ok(Some(records))
    }

    pub async fn create(
        &self,
        actor: WebhookActor<'_>,
        project_id: &str,
        webhook: NewWebhook,
    ) -> Result<WebhookMutationOutcome> {
        let Some(role) = self.project_role(actor, project_id).await? else {
            return Ok(WebhookMutationOutcome::Missing);
        };
        if !actor.is_admin && !role.can_manage_services() {
            return Ok(WebhookMutationOutcome::Forbidden);
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            "INSERT INTO project_webhooks (id, project_id, name, url, secret_ciphertext, is_enabled, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(project_id)
        .bind(&webhook.name)
        .bind(&webhook.url)
        .bind(&webhook.secret_ciphertext)
        .bind(webhook.is_enabled)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await;
        if let Err(error) = inserted {
            if let sqlx::Error::Database(database_error) = &error
                && database_error.is_unique_violation()
            {
                return Err(DatabaseError::WebhookNameConflict);
            }
            return Err(error.into());
        }
        insert_audit(&mut tx, actor.id, "webhook.create", &id, &now).await?;
        let record = row(&mut tx, &id).await?.ok_or(sqlx::Error::RowNotFound)?;
        tx.commit().await?;
        Ok(WebhookMutationOutcome::Created(record))
    }

    pub async fn remove(
        &self,
        actor: WebhookActor<'_>,
        webhook_id: &str,
        confirm_name: &str,
    ) -> Result<WebhookMutationOutcome> {
        let Some((record, role)) = self.get_with_role(actor, webhook_id).await? else {
            return Ok(WebhookMutationOutcome::Missing);
        };
        if !actor.is_admin && !role.can_manage_services() {
            return Ok(WebhookMutationOutcome::Forbidden);
        }
        if record.name != confirm_name {
            return Err(DatabaseError::WebhookConfirmationMismatch);
        }
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM project_webhooks WHERE id = ?")
            .bind(webhook_id)
            .execute(&mut *tx)
            .await?;
        insert_audit(&mut tx, actor.id, "webhook.delete", webhook_id, &now).await?;
        tx.commit().await?;
        Ok(WebhookMutationOutcome::Removed(record))
    }

    async fn get_with_role(
        &self,
        actor: WebhookActor<'_>,
        webhook_id: &str,
    ) -> Result<Option<(WebhookRecord, ignitify_domain::ProjectMemberRole)>> {
        let row = sqlx::query_as::<_, WebhookRow>(
            "SELECT id, project_id, name, url, secret_ciphertext, is_enabled, created_at, updated_at
             FROM project_webhooks WHERE id = ?",
        )
        .bind(webhook_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let Some(role) = self.project_role(actor, &row.project_id).await? else {
            return Ok(None);
        };
        Ok(Some((row.into(), role)))
    }

    async fn project_role(
        &self,
        actor: WebhookActor<'_>,
        project_id: &str,
    ) -> Result<Option<ignitify_domain::ProjectMemberRole>> {
        if actor.is_admin {
            let exists: Option<String> = sqlx::query_scalar("SELECT id FROM projects WHERE id = ?")
                .bind(project_id)
                .fetch_optional(&self.pool)
                .await?;
            return Ok(exists.map(|_| ignitify_domain::ProjectMemberRole::Owner));
        }
        let role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM project_members WHERE project_id = ? AND user_id = ?",
        )
        .bind(project_id)
        .bind(actor.id)
        .fetch_optional(&self.pool)
        .await?;
        role.map(|role| {
            role.as_str()
                .try_into()
                .map_err(|_| DatabaseError::InvalidProjectMemberRole(role))
        })
        .transpose()
    }
}

async fn row(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    webhook_id: &str,
) -> Result<Option<WebhookRecord>> {
    let record = sqlx::query_as::<_, WebhookRow>(
        "SELECT id, project_id, name, url, secret_ciphertext, is_enabled, created_at, updated_at
         FROM project_webhooks WHERE id = ?",
    )
    .bind(webhook_id)
    .fetch_optional(&mut **tx)
    .await?
    .map(WebhookRecord::from);
    Ok(record)
}

async fn insert_audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    actor_id: &str,
    action: &str,
    webhook_id: &str,
    now: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO audit_logs (id, user_id, action, resource_type, resource_id, created_at)
         VALUES (?, ?, ?, 'webhook', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(actor_id)
    .bind(action)
    .bind(webhook_id)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[derive(Debug, FromRow)]
struct WebhookRow {
    id: String,
    project_id: String,
    name: String,
    url: String,
    secret_ciphertext: Option<String>,
    is_enabled: bool,
    created_at: String,
    updated_at: String,
}

impl From<WebhookRow> for WebhookRecord {
    fn from(row: WebhookRow) -> Self {
        Self {
            id: row.id,
            project_id: row.project_id,
            name: row.name,
            url: row.url,
            secret_ciphertext: row.secret_ciphertext,
            is_enabled: row.is_enabled,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
