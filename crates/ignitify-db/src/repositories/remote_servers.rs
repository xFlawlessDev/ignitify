use chrono::Utc;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::{DatabaseError, Result};

#[derive(Debug, Clone)]
pub struct RemoteServerRecord {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: i64,
    pub username: String,
    pub deploy_path: String,
    pub public_key_configured: bool,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct RemoteServerConnection {
    pub id: String,
    pub host: String,
    pub port: i64,
    pub username: String,
    pub deploy_path: String,
    pub private_key_ciphertext: String,
    pub public_key_ciphertext: String,
    pub known_hosts_ciphertext: String,
}

#[derive(Debug, Clone)]
pub struct NewRemoteServer {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub deploy_path: String,
    pub private_key_ciphertext: String,
    pub public_key_ciphertext: String,
    pub known_hosts_ciphertext: String,
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct RemoteServerUpdate {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub deploy_path: String,
    pub private_key_ciphertext: Option<String>,
    pub public_key_ciphertext: Option<String>,
    pub known_hosts_ciphertext: Option<String>,
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct RemoteServersRepository {
    pool: SqlitePool,
}

impl RemoteServersRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<RemoteServerRecord>> {
        let rows = sqlx::query_as::<_, RemoteServerRow>(
            "SELECT id, name, host, port, username, deploy_path,
                    (length(public_key_ciphertext) > 0) AS public_key_configured,
                    is_default, created_at, updated_at
             FROM remote_servers
             ORDER BY is_default DESC, name COLLATE NOCASE",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(RemoteServerRow::into_record).collect())
    }

    pub async fn create(&self, input: NewRemoteServer) -> Result<RemoteServerRecord> {
        let mut transaction = self.pool.begin().await?;
        if input.is_default {
            sqlx::query("UPDATE remote_servers SET is_default = 0 WHERE is_default = 1")
                .execute(&mut *transaction)
                .await?;
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "INSERT INTO remote_servers
             (id, name, host, port, username, deploy_path, private_key_ciphertext,
              public_key_ciphertext, known_hosts_ciphertext, is_default, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&input.host)
        .bind(i64::from(input.port))
        .bind(&input.username)
        .bind(&input.deploy_path)
        .bind(&input.private_key_ciphertext)
        .bind(&input.public_key_ciphertext)
        .bind(&input.known_hosts_ciphertext)
        .bind(input.is_default)
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await;
        if let Err(error) = result {
            return map_name_conflict(error);
        }
        transaction.commit().await?;
        self.get(&id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound.into())
    }

    pub async fn update(
        &self,
        id: &str,
        input: RemoteServerUpdate,
    ) -> Result<Option<RemoteServerRecord>> {
        let mut transaction = self.pool.begin().await?;
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM remote_servers WHERE id = ?)",
        )
        .bind(id)
        .fetch_one(&mut *transaction)
        .await?;
        if exists == 0 {
            transaction.rollback().await?;
            return Ok(None);
        }
        if input.is_default {
            sqlx::query("UPDATE remote_servers SET is_default = 0 WHERE is_default = 1")
                .execute(&mut *transaction)
                .await?;
        }
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE remote_servers
             SET name = ?, host = ?, port = ?, username = ?, deploy_path = ?,
                 private_key_ciphertext = COALESCE(?, private_key_ciphertext),
                 public_key_ciphertext = COALESCE(?, public_key_ciphertext),
                 known_hosts_ciphertext = COALESCE(?, known_hosts_ciphertext),
                 is_default = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&input.name)
        .bind(&input.host)
        .bind(i64::from(input.port))
        .bind(&input.username)
        .bind(&input.deploy_path)
        .bind(&input.private_key_ciphertext)
        .bind(&input.public_key_ciphertext)
        .bind(&input.known_hosts_ciphertext)
        .bind(input.is_default)
        .bind(&now)
        .bind(id)
        .execute(&mut *transaction)
        .await;
        if let Err(error) = result {
            return map_name_conflict(error);
        }
        transaction.commit().await?;
        self.get(id).await
    }

    pub async fn delete(&self, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM remote_servers WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn set_default(&self, id: &str) -> Result<Option<RemoteServerRecord>> {
        let mut transaction = self.pool.begin().await?;
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM remote_servers WHERE id = ?)",
        )
        .bind(id)
        .fetch_one(&mut *transaction)
        .await?;
        if exists == 0 {
            transaction.rollback().await?;
            return Ok(None);
        }
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE remote_servers SET is_default = 0 WHERE is_default = 1")
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE remote_servers SET is_default = 1, updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        self.get(id).await
    }

    pub async fn active(&self) -> Result<Option<RemoteServerConnection>> {
        let row = sqlx::query_as::<_, RemoteServerConnectionRow>(
            "SELECT id, host, port, username, deploy_path, private_key_ciphertext,
                    public_key_ciphertext, known_hosts_ciphertext
             FROM remote_servers
             WHERE is_default = 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(RemoteServerConnectionRow::into_connection))
    }

    pub async fn connection(&self, id: &str) -> Result<Option<RemoteServerConnection>> {
        let row = sqlx::query_as::<_, RemoteServerConnectionRow>(
            "SELECT id, host, port, username, deploy_path, private_key_ciphertext,
                    public_key_ciphertext, known_hosts_ciphertext
             FROM remote_servers
             WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(RemoteServerConnectionRow::into_connection))
    }

    async fn get(&self, id: &str) -> Result<Option<RemoteServerRecord>> {
        let row = sqlx::query_as::<_, RemoteServerRow>(
            "SELECT id, name, host, port, username, deploy_path,
                    (length(public_key_ciphertext) > 0) AS public_key_configured,
                    is_default, created_at, updated_at
             FROM remote_servers
             WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(RemoteServerRow::into_record))
    }
}

fn map_name_conflict<T>(error: sqlx::Error) -> Result<T> {
    match error {
        sqlx::Error::Database(database_error) if database_error.is_unique_violation() => {
            Err(DatabaseError::RemoteServerNameConflict)
        }
        error => Err(error.into()),
    }
}

#[derive(Debug, FromRow)]
struct RemoteServerRow {
    id: String,
    name: String,
    host: String,
    port: i64,
    username: String,
    deploy_path: String,
    public_key_configured: i64,
    is_default: i64,
    created_at: String,
    updated_at: String,
}

impl RemoteServerRow {
    fn into_record(self) -> RemoteServerRecord {
        RemoteServerRecord {
            id: self.id,
            name: self.name,
            host: self.host,
            port: self.port,
            username: self.username,
            deploy_path: self.deploy_path,
            public_key_configured: self.public_key_configured != 0,
            is_default: self.is_default != 0,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct RemoteServerConnectionRow {
    id: String,
    host: String,
    port: i64,
    username: String,
    deploy_path: String,
    private_key_ciphertext: String,
    public_key_ciphertext: String,
    known_hosts_ciphertext: String,
}

impl RemoteServerConnectionRow {
    fn into_connection(self) -> RemoteServerConnection {
        RemoteServerConnection {
            id: self.id,
            host: self.host,
            port: self.port,
            username: self.username,
            deploy_path: self.deploy_path,
            private_key_ciphertext: self.private_key_ciphertext,
            public_key_ciphertext: self.public_key_ciphertext,
            known_hosts_ciphertext: self.known_hosts_ciphertext,
        }
    }
}
