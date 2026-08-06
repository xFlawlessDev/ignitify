use chrono::Utc;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::{DatabaseError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Git,
    Gitea,
    Gitlab,
    Github,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Gitea => "gitea",
            Self::Gitlab => "gitlab",
            Self::Github => "github",
        }
    }
}

impl TryFrom<&str> for ProviderKind {
    type Error = DatabaseError;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "git" => Ok(Self::Git),
            "gitea" => Ok(Self::Gitea),
            "gitlab" => Ok(Self::Gitlab),
            "github" => Ok(Self::Github),
            other => Err(DatabaseError::InvalidProviderKind(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderAuthMode {
    Token,
    OAuth,
    GithubApp,
}

impl ProviderAuthMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Token => "token",
            Self::OAuth => "oauth",
            Self::GithubApp => "github_app",
        }
    }
}

impl TryFrom<&str> for ProviderAuthMode {
    type Error = DatabaseError;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "token" => Ok(Self::Token),
            "oauth" => Ok(Self::OAuth),
            "github_app" => Ok(Self::GithubApp),
            other => Err(DatabaseError::InvalidProviderAuthMode(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewProvider {
    pub name: String,
    pub kind: ProviderKind,
    pub auth_mode: ProviderAuthMode,
    pub base_url: String,
    pub internal_url: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: Option<String>,
    pub application_id: Option<String>,
    pub installation_id: Option<String>,
    pub group_names: Option<String>,
    pub username: Option<String>,
    pub credentials_ciphertext: String,
}

#[derive(Debug, Clone)]
pub struct ProviderRecord {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub auth_mode: ProviderAuthMode,
    pub base_url: String,
    pub internal_url: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: Option<String>,
    pub application_id: Option<String>,
    pub installation_id: Option<String>,
    pub group_names: Option<String>,
    pub username: Option<String>,
    pub credentials_ciphertext: String,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_verified_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProviderUpdate {
    pub name: String,
    pub kind: ProviderKind,
    pub auth_mode: ProviderAuthMode,
    pub base_url: String,
    pub internal_url: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: Option<String>,
    pub application_id: Option<String>,
    pub installation_id: Option<String>,
    pub group_names: Option<String>,
    pub username: Option<String>,
    pub credentials_ciphertext: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ProviderMutationOutcome {
    Updated(Box<ProviderRecord>),
    Missing,
}

#[derive(Debug, Clone)]
pub struct ProvidersRepository {
    pool: SqlitePool,
}

impl ProvidersRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<ProviderRecord>> {
        let rows = sqlx::query_as::<_, ProviderRow>(
            "SELECT id, name, kind, auth_mode, base_url, internal_url, redirect_uri, client_id,
                    application_id, installation_id, group_names, username, token_ciphertext, created_by,
                    created_at, updated_at, last_verified_at
             FROM providers
             ORDER BY name COLLATE NOCASE",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(ProviderRow::into_record).collect()
    }

    pub async fn create(&self, actor_id: &str, input: NewProvider) -> Result<ProviderRecord> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "INSERT INTO providers
             (id, name, kind, auth_mode, base_url, internal_url, redirect_uri, client_id,
              application_id, installation_id, group_names, username, token_ciphertext,
              created_by, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.name)
        .bind(input.kind.as_str())
        .bind(input.auth_mode.as_str())
        .bind(&input.base_url)
        .bind(&input.internal_url)
        .bind(&input.redirect_uri)
        .bind(&input.client_id)
        .bind(&input.application_id)
        .bind(&input.installation_id)
        .bind(&input.group_names)
        .bind(&input.username)
        .bind(&input.credentials_ciphertext)
        .bind(actor_id)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await;
        if let Err(error) = result {
            return match error {
                sqlx::Error::Database(database_error) if database_error.is_unique_violation() => {
                    Err(DatabaseError::ProviderNameConflict)
                }
                error => Err(error.into()),
            };
        }
        self.get(&id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound.into())
    }

    pub async fn get(&self, provider_id: &str) -> Result<Option<ProviderRecord>> {
        sqlx::query_as::<_, ProviderRow>(
            "SELECT id, name, kind, auth_mode, base_url, internal_url, redirect_uri, client_id,
                    application_id, installation_id, group_names, username, token_ciphertext, created_by,
                    created_at, updated_at, last_verified_at
             FROM providers WHERE id = ?",
        )
        .bind(provider_id)
        .fetch_optional(&self.pool)
        .await?
        .map(ProviderRow::into_record)
        .transpose()
    }

    pub async fn update(
        &self,
        provider_id: &str,
        input: ProviderUpdate,
    ) -> Result<ProviderMutationOutcome> {
        let Some(current) = self.get(provider_id).await? else {
            return Ok(ProviderMutationOutcome::Missing);
        };
        let now = Utc::now().to_rfc3339();
        let ciphertext = input
            .credentials_ciphertext
            .unwrap_or(current.credentials_ciphertext);
        let result = sqlx::query(
            "UPDATE providers SET name = ?, kind = ?, auth_mode = ?, base_url = ?,
                    internal_url = ?, redirect_uri = ?, client_id = ?, application_id = ?,
                    installation_id = ?, group_names = ?, username = ?, token_ciphertext = ?,
                    updated_at = ?, last_verified_at = NULL
             WHERE id = ?",
        )
        .bind(&input.name)
        .bind(input.kind.as_str())
        .bind(input.auth_mode.as_str())
        .bind(&input.base_url)
        .bind(&input.internal_url)
        .bind(&input.redirect_uri)
        .bind(&input.client_id)
        .bind(&input.application_id)
        .bind(&input.installation_id)
        .bind(&input.group_names)
        .bind(&input.username)
        .bind(ciphertext)
        .bind(&now)
        .bind(provider_id)
        .execute(&self.pool)
        .await;
        if let Err(error) = result {
            return match error {
                sqlx::Error::Database(database_error) if database_error.is_unique_violation() => {
                    Err(DatabaseError::ProviderNameConflict)
                }
                error => Err(error.into()),
            };
        }
        self.get(provider_id)
            .await?
            .map(|provider| ProviderMutationOutcome::Updated(Box::new(provider)))
            .ok_or_else(|| sqlx::Error::RowNotFound.into())
    }

    pub async fn delete(&self, provider_id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM providers WHERE id = ?")
            .bind(provider_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn mark_verified(&self, provider_id: &str) -> Result<Option<ProviderRecord>> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query("UPDATE providers SET last_verified_at = ? WHERE id = ?")
            .bind(&now)
            .bind(provider_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() != 1 {
            return Ok(None);
        }
        self.get(provider_id).await
    }
}

#[derive(Debug, FromRow)]
struct ProviderRow {
    id: String,
    name: String,
    kind: String,
    auth_mode: String,
    base_url: String,
    internal_url: Option<String>,
    redirect_uri: Option<String>,
    client_id: Option<String>,
    application_id: Option<String>,
    installation_id: Option<String>,
    group_names: Option<String>,
    username: Option<String>,
    token_ciphertext: String,
    created_by: String,
    created_at: String,
    updated_at: String,
    last_verified_at: Option<String>,
}

impl ProviderRow {
    fn into_record(self) -> Result<ProviderRecord> {
        Ok(ProviderRecord {
            id: self.id,
            name: self.name,
            kind: ProviderKind::try_from(self.kind.as_str())?,
            auth_mode: ProviderAuthMode::try_from(self.auth_mode.as_str())?,
            base_url: self.base_url,
            internal_url: self.internal_url,
            redirect_uri: self.redirect_uri,
            client_id: self.client_id,
            application_id: self.application_id,
            installation_id: self.installation_id,
            group_names: self.group_names,
            username: self.username,
            credentials_ciphertext: self.token_ciphertext,
            created_by: self.created_by,
            created_at: self.created_at,
            updated_at: self.updated_at,
            last_verified_at: self.last_verified_at,
        })
    }
}
