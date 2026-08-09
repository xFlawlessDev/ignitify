use chrono::Utc;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::{DatabaseError, Result};

#[derive(Debug, Clone)]
pub struct RemoteBuilderRecord {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub registry_repository: String,
    pub tls_server_name: Option<String>,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct RemoteBuilderConnection {
    pub id: String,
    pub endpoint: String,
    pub registry_repository: String,
    pub tls_server_name: Option<String>,
    pub ca_certificate_ciphertext: String,
    pub client_certificate_ciphertext: String,
    pub client_key_ciphertext: String,
}

#[derive(Debug, Clone)]
pub struct NewRemoteBuilder {
    pub name: String,
    pub endpoint: String,
    pub registry_repository: String,
    pub tls_server_name: Option<String>,
    pub ca_certificate_ciphertext: String,
    pub client_certificate_ciphertext: String,
    pub client_key_ciphertext: String,
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct RemoteBuilderUpdate {
    pub name: String,
    pub endpoint: String,
    pub registry_repository: String,
    pub tls_server_name: Option<String>,
    pub ca_certificate_ciphertext: String,
    pub client_certificate_ciphertext: String,
    pub client_key_ciphertext: String,
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct RemoteBuildersRepository {
    pool: SqlitePool,
}

impl RemoteBuildersRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<RemoteBuilderRecord>> {
        let rows = sqlx::query_as::<_, RemoteBuilderRow>(
            "SELECT id, name, endpoint, registry_repository, tls_server_name, is_default,
                    created_at, updated_at
             FROM remote_builders
             ORDER BY is_default DESC, name COLLATE NOCASE",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(RemoteBuilderRow::into_record)
            .collect())
    }

    pub async fn create(&self, input: NewRemoteBuilder) -> Result<RemoteBuilderRecord> {
        let mut transaction = self.pool.begin().await?;
        if input.is_default {
            sqlx::query("UPDATE remote_builders SET is_default = 0 WHERE is_default = 1")
                .execute(&mut *transaction)
                .await?;
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "INSERT INTO remote_builders
             (id, name, endpoint, registry_repository, tls_server_name,
              ca_certificate_ciphertext, client_certificate_ciphertext, client_key_ciphertext,
              is_default, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&input.endpoint)
        .bind(&input.registry_repository)
        .bind(&input.tls_server_name)
        .bind(&input.ca_certificate_ciphertext)
        .bind(&input.client_certificate_ciphertext)
        .bind(&input.client_key_ciphertext)
        .bind(input.is_default)
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await;
        if let Err(error) = result {
            return match error {
                sqlx::Error::Database(database_error) if database_error.is_unique_violation() => {
                    Err(DatabaseError::RemoteBuilderNameConflict)
                }
                error => Err(error.into()),
            };
        }
        transaction.commit().await?;
        self.get(&id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound.into())
    }

    pub async fn update(
        &self,
        id: &str,
        input: RemoteBuilderUpdate,
    ) -> Result<Option<RemoteBuilderRecord>> {
        let mut transaction = self.pool.begin().await?;
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM remote_builders WHERE id = ?)",
        )
        .bind(id)
        .fetch_one(&mut *transaction)
        .await?;
        if exists == 0 {
            transaction.rollback().await?;
            return Ok(None);
        }
        if input.is_default {
            sqlx::query("UPDATE remote_builders SET is_default = 0 WHERE is_default = 1")
                .execute(&mut *transaction)
                .await?;
        }
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE remote_builders
             SET name = ?, endpoint = ?, registry_repository = ?, tls_server_name = ?,
                 ca_certificate_ciphertext = ?, client_certificate_ciphertext = ?,
                 client_key_ciphertext = ?, is_default = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&input.name)
        .bind(&input.endpoint)
        .bind(&input.registry_repository)
        .bind(&input.tls_server_name)
        .bind(&input.ca_certificate_ciphertext)
        .bind(&input.client_certificate_ciphertext)
        .bind(&input.client_key_ciphertext)
        .bind(input.is_default)
        .bind(&now)
        .bind(id)
        .execute(&mut *transaction)
        .await;
        if let Err(error) = result {
            return match error {
                sqlx::Error::Database(database_error) if database_error.is_unique_violation() => {
                    Err(DatabaseError::RemoteBuilderNameConflict)
                }
                error => Err(error.into()),
            };
        }
        transaction.commit().await?;
        self.get(id).await
    }

    pub async fn delete(&self, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM remote_builders WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn set_default(&self, id: &str) -> Result<Option<RemoteBuilderRecord>> {
        let mut transaction = self.pool.begin().await?;
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM remote_builders WHERE id = ?)",
        )
        .bind(id)
        .fetch_one(&mut *transaction)
        .await?;
        if exists == 0 {
            transaction.rollback().await?;
            return Ok(None);
        }
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE remote_builders SET is_default = 0 WHERE is_default = 1")
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE remote_builders SET is_default = 1, updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        self.get(id).await
    }

    pub async fn active(&self) -> Result<Option<RemoteBuilderConnection>> {
        let row = sqlx::query_as::<_, RemoteBuilderConnectionRow>(
            "SELECT id, endpoint, registry_repository, tls_server_name,
                    ca_certificate_ciphertext, client_certificate_ciphertext,
                    client_key_ciphertext
             FROM remote_builders WHERE is_default = 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(RemoteBuilderConnectionRow::into_connection))
    }

    async fn get(&self, id: &str) -> Result<Option<RemoteBuilderRecord>> {
        let row = sqlx::query_as::<_, RemoteBuilderRow>(
            "SELECT id, name, endpoint, registry_repository, tls_server_name, is_default,
                    created_at, updated_at
             FROM remote_builders WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(RemoteBuilderRow::into_record))
    }
}

#[derive(Debug, FromRow)]
struct RemoteBuilderRow {
    id: String,
    name: String,
    endpoint: String,
    registry_repository: String,
    tls_server_name: Option<String>,
    is_default: i64,
    created_at: String,
    updated_at: String,
}

impl RemoteBuilderRow {
    fn into_record(self) -> RemoteBuilderRecord {
        RemoteBuilderRecord {
            id: self.id,
            name: self.name,
            endpoint: self.endpoint,
            registry_repository: self.registry_repository,
            tls_server_name: self.tls_server_name,
            is_default: self.is_default != 0,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct RemoteBuilderConnectionRow {
    id: String,
    endpoint: String,
    registry_repository: String,
    tls_server_name: Option<String>,
    ca_certificate_ciphertext: String,
    client_certificate_ciphertext: String,
    client_key_ciphertext: String,
}

impl RemoteBuilderConnectionRow {
    fn into_connection(self) -> RemoteBuilderConnection {
        RemoteBuilderConnection {
            id: self.id,
            endpoint: self.endpoint,
            registry_repository: self.registry_repository,
            tls_server_name: self.tls_server_name,
            ca_certificate_ciphertext: self.ca_certificate_ciphertext,
            client_certificate_ciphertext: self.client_certificate_ciphertext,
            client_key_ciphertext: self.client_key_ciphertext,
        }
    }
}
