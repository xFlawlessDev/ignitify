use chrono::Utc;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::{DatabaseError, Result};

#[derive(Debug, Clone)]
pub struct ServerSettingsRecord {
    pub application_domain_suffix: String,
    pub https_enabled: bool,
    pub automatically_provision_ssl: bool,
    pub acme_email: String,
    pub dns_record_type: String,
    pub dns_record_target: String,
    pub certificate_provider: String,
    pub custom_certificate_id: Option<String>,
    pub concurrent_builds: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct ServerSettingsUpdate {
    pub application_domain_suffix: String,
    pub https_enabled: bool,
    pub automatically_provision_ssl: bool,
    pub acme_email: String,
    pub dns_record_type: String,
    pub dns_record_target: String,
    pub certificate_provider: String,
    pub custom_certificate_id: Option<String>,
    pub concurrent_builds: i64,
}

#[derive(Debug, Clone)]
pub struct NewServerCertificate {
    pub name: String,
    pub certificate_file_name: String,
    pub private_key_file_name: String,
    pub certificate_ciphertext: String,
    pub private_key_ciphertext: String,
}

#[derive(Debug, Clone)]
pub struct ServerCertificateRecord {
    pub id: String,
    pub name: String,
    pub certificate_file_name: String,
    pub private_key_file_name: String,
    pub certificate_ciphertext: String,
    pub private_key_ciphertext: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct ServerSettingsRepository {
    pool: SqlitePool,
}

impl ServerSettingsRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get(&self) -> Result<ServerSettingsRecord> {
        let row = sqlx::query_as::<_, ServerSettingsRow>(
            "SELECT server_domain AS application_domain_suffix, https_enabled, automatically_provision_ssl,
                    acme_email,
                    dns_record_type, dns_record_target,
                    certificate_provider, custom_certificate_id, concurrent_builds, updated_at
             FROM server_settings WHERE id = 1",
        )
        .fetch_one(&self.pool)
        .await?;
        row.into_record()
    }

    pub async fn update(&self, input: ServerSettingsUpdate) -> Result<ServerSettingsRecord> {
        if !(1..=32).contains(&input.concurrent_builds) {
            return Err(DatabaseError::InvalidConcurrentBuilds);
        }
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE server_settings
             SET server_domain = ?, https_enabled = ?, automatically_provision_ssl = ?,
                 acme_email = ?, dns_record_type = ?, dns_record_target = ?,
                 certificate_provider = ?, custom_certificate_id = ?, concurrent_builds = ?,
                 updated_at = ?
             WHERE id = 1",
        )
        .bind(&input.application_domain_suffix)
        .bind(input.https_enabled)
        .bind(input.automatically_provision_ssl)
        .bind(&input.acme_email)
        .bind(&input.dns_record_type)
        .bind(&input.dns_record_target)
        .bind(&input.certificate_provider)
        .bind(&input.custom_certificate_id)
        .bind(input.concurrent_builds)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.get().await
    }

    pub async fn list_certificates(&self) -> Result<Vec<ServerCertificateRecord>> {
        let rows = sqlx::query_as::<_, ServerCertificateRow>(
            "SELECT id, name, certificate_file_name, private_key_file_name,
                    certificate_ciphertext, private_key_ciphertext, created_at, updated_at
             FROM server_certificates ORDER BY name COLLATE NOCASE",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(ServerCertificateRow::into_record)
            .collect())
    }

    pub async fn create_certificate(
        &self,
        input: NewServerCertificate,
    ) -> Result<ServerCertificateRecord> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO server_certificates
             (id, name, certificate_file_name, private_key_file_name,
              certificate_ciphertext, private_key_ciphertext, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&input.certificate_file_name)
        .bind(&input.private_key_file_name)
        .bind(&input.certificate_ciphertext)
        .bind(&input.private_key_ciphertext)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.certificate(&id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound.into())
    }

    pub async fn delete_certificate(&self, certificate_id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM server_certificates WHERE id = ?")
            .bind(certificate_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn certificate_exists(&self, certificate_id: &str) -> Result<bool> {
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM server_certificates WHERE id = ?)",
        )
        .bind(certificate_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists == 1)
    }

    pub async fn certificate(
        &self,
        certificate_id: &str,
    ) -> Result<Option<ServerCertificateRecord>> {
        let row = sqlx::query_as::<_, ServerCertificateRow>(
            "SELECT id, name, certificate_file_name, private_key_file_name,
                    certificate_ciphertext, private_key_ciphertext, created_at, updated_at
             FROM server_certificates WHERE id = ?",
        )
        .bind(certificate_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(ServerCertificateRow::into_record))
    }
}

#[derive(Debug, FromRow)]
struct ServerSettingsRow {
    application_domain_suffix: String,
    https_enabled: i64,
    automatically_provision_ssl: i64,
    acme_email: String,
    dns_record_type: String,
    dns_record_target: String,
    certificate_provider: String,
    custom_certificate_id: Option<String>,
    concurrent_builds: i64,
    updated_at: String,
}

impl ServerSettingsRow {
    fn into_record(self) -> Result<ServerSettingsRecord> {
        if !(1..=32).contains(&self.concurrent_builds) {
            return Err(DatabaseError::InvalidConcurrentBuilds);
        }
        if !matches!(
            self.certificate_provider.as_str(),
            "none" | "lets-encrypt" | "custom"
        ) {
            return Err(DatabaseError::InvalidCertificateProvider(
                self.certificate_provider,
            ));
        }
        Ok(ServerSettingsRecord {
            application_domain_suffix: self.application_domain_suffix,
            https_enabled: self.https_enabled != 0,
            automatically_provision_ssl: self.automatically_provision_ssl != 0,
            acme_email: self.acme_email,
            dns_record_type: self.dns_record_type,
            dns_record_target: self.dns_record_target,
            certificate_provider: self.certificate_provider,
            custom_certificate_id: self.custom_certificate_id,
            concurrent_builds: self.concurrent_builds,
            updated_at: self.updated_at,
        })
    }
}

#[derive(Debug, FromRow)]
struct ServerCertificateRow {
    id: String,
    name: String,
    certificate_file_name: String,
    private_key_file_name: String,
    certificate_ciphertext: String,
    private_key_ciphertext: String,
    created_at: String,
    updated_at: String,
}

impl ServerCertificateRow {
    fn into_record(self) -> ServerCertificateRecord {
        ServerCertificateRecord {
            id: self.id,
            name: self.name,
            certificate_file_name: self.certificate_file_name,
            private_key_file_name: self.private_key_file_name,
            certificate_ciphertext: self.certificate_ciphertext,
            private_key_ciphertext: self.private_key_ciphertext,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
