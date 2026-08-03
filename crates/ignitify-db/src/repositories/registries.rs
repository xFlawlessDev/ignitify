use chrono::Utc;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::{DatabaseError, Result};

#[derive(Debug, Clone, Copy)]
pub struct RegistryActor<'a> {
    pub is_admin: bool,
    pub user_id: &'a str,
}

#[derive(Debug, Clone)]
pub struct NewRegistry {
    pub name: String,
    pub endpoint: String,
    pub username: Option<String>,
    pub credential_ciphertext: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegistryRecord {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub username: Option<String>,
    pub credential_ciphertext: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct RegistriesRepository {
    pool: SqlitePool,
}

impl RegistriesRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, actor: RegistryActor<'_>) -> Result<Option<Vec<RegistryRecord>>> {
        if !actor.is_admin {
            return Ok(None);
        }
        rows(&self.pool).await.map(Some)
    }

    pub async fn create(
        &self,
        actor: RegistryActor<'_>,
        registry: NewRegistry,
    ) -> Result<Option<RegistryRecord>> {
        if !actor.is_admin {
            return Ok(None);
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            "INSERT INTO registries (id, name, endpoint, username, credential_ciphertext, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&registry.name)
        .bind(&registry.endpoint)
        .bind(&registry.username)
        .bind(&registry.credential_ciphertext)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await;
        if let Err(error) = inserted {
            if let sqlx::Error::Database(database_error) = &error
                && database_error.is_unique_violation()
            {
                return Err(DatabaseError::RegistryNameConflict);
            }
            return Err(error.into());
        }
        insert_audit(&mut tx, actor.user_id, "registry.create", &id, &now).await?;
        let record = row(&mut tx, &id).await?.ok_or(sqlx::Error::RowNotFound)?;
        tx.commit().await?;
        Ok(Some(record))
    }

    pub async fn delete(
        &self,
        actor: RegistryActor<'_>,
        registry_id: &str,
        confirm_name: &str,
    ) -> Result<Option<RegistryRecord>> {
        if !actor.is_admin {
            return Ok(None);
        }
        let mut tx = self.pool.begin().await?;
        let record = row(&mut tx, registry_id).await?;
        let Some(record) = record else {
            tx.commit().await?;
            return Ok(None);
        };
        if record.name != confirm_name {
            return Err(DatabaseError::RegistryConfirmationMismatch);
        }
        sqlx::query("DELETE FROM registries WHERE id = ?")
            .bind(registry_id)
            .execute(&mut *tx)
            .await?;
        insert_audit(
            &mut tx,
            actor.user_id,
            "registry.delete",
            registry_id,
            &Utc::now().to_rfc3339(),
        )
        .await?;
        tx.commit().await?;
        Ok(Some(record))
    }
}

async fn rows(pool: &SqlitePool) -> Result<Vec<RegistryRecord>> {
    let records = sqlx::query_as::<_, RegistryRow>(
        "SELECT id, name, endpoint, username, credential_ciphertext, created_at, updated_at
         FROM registries ORDER BY name",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(RegistryRecord::from)
    .collect();
    Ok(records)
}

async fn row(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    registry_id: &str,
) -> Result<Option<RegistryRecord>> {
    let record = sqlx::query_as::<_, RegistryRow>(
        "SELECT id, name, endpoint, username, credential_ciphertext, created_at, updated_at
         FROM registries WHERE id = ?",
    )
    .bind(registry_id)
    .fetch_optional(&mut **tx)
    .await?
    .map(RegistryRecord::from);
    Ok(record)
}

async fn insert_audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    actor_id: &str,
    action: &str,
    registry_id: &str,
    now: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO audit_logs (id, user_id, action, resource_type, resource_id, created_at)
         VALUES (?, ?, ?, 'registry', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(actor_id)
    .bind(action)
    .bind(registry_id)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[derive(Debug, FromRow)]
struct RegistryRow {
    id: String,
    name: String,
    endpoint: String,
    username: Option<String>,
    credential_ciphertext: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<RegistryRow> for RegistryRecord {
    fn from(row: RegistryRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            endpoint: row.endpoint,
            username: row.username,
            credential_ciphertext: row.credential_ciphertext,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
