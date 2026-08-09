use chrono::Utc;
use ignitify_domain::{
    DnsRecord, DnsVerificationStatus, DomainId, DomainName, DomainStatus, ProjectMemberRole,
    ServiceId,
};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::{DatabaseError, Result};

#[derive(Debug, Clone, Copy)]
pub struct DomainActor<'a> {
    pub id: &'a str,
    pub is_admin: bool,
}

#[derive(Debug, Clone)]
pub struct DomainRecord {
    pub id: DomainId,
    pub service_id: ServiceId,
    pub hostname: DomainName,
    pub status: DomainStatus,
    pub last_error: Option<String>,
    pub dns_record: Option<DnsRecord>,
    pub dns_status: DnsVerificationStatus,
    pub dns_error: Option<String>,
    pub dns_checked_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub enum DomainMutationOutcome {
    Created(DomainRecord),
    Removed(DomainRecord),
    Missing,
    Forbidden,
}

#[derive(Debug, Clone)]
pub enum DomainVerificationRequestOutcome {
    Requested(Box<DomainRecord>),
    NotConfigured,
    Missing,
    Forbidden,
}

#[derive(Debug, Clone)]
pub struct DomainsRepository {
    pool: SqlitePool,
}

impl DomainsRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(
        &self,
        actor: DomainActor<'_>,
        service_id: &str,
    ) -> Result<Option<Vec<DomainRecord>>> {
        if self.service_role(actor, service_id).await?.is_none() {
            return Ok(None);
        }
        let rows = sqlx::query_as::<_, DomainRow>(
            "SELECT id, service_id, hostname, status, last_error,
                    dns_record_type, dns_record_target, dns_status, dns_error, dns_checked_at,
                    created_at, updated_at
             FROM domains WHERE service_id = ? ORDER BY created_at",
        )
        .bind(service_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(domain_from_row)
            .collect::<Result<_>>()
            .map(Some)
    }

    pub async fn create(
        &self,
        actor: DomainActor<'_>,
        service_id: &str,
        hostname: DomainName,
        dns_record: DnsRecord,
    ) -> Result<DomainMutationOutcome> {
        let Some(role) = self.service_role(actor, service_id).await? else {
            return Ok(DomainMutationOutcome::Missing);
        };
        if !actor.is_admin && !role.can_manage_services() {
            return Ok(DomainMutationOutcome::Forbidden);
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let result = sqlx::query(
            "INSERT INTO domains
             (id, service_id, hostname, status, dns_record_type, dns_record_target, dns_status, created_at, updated_at)
             VALUES (?, ?, ?, 'pending', ?, ?, 'not_checked', ?, ?)",
        )
        .bind(&id)
        .bind(service_id)
        .bind(hostname.as_str())
        .bind(dns_record.record_type().as_str())
        .bind(dns_record.target().to_string())
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await;
        if let Err(error) = result {
            if let sqlx::Error::Database(database_error) = &error
                && database_error.is_unique_violation()
            {
                return Err(DatabaseError::DomainNameConflict);
            }
            return Err(error.into());
        }
        insert_audit(&mut tx, actor.id, "domain.create", &id, &now).await?;
        let record = fetch_domain(&mut tx, &id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
        tx.commit().await?;
        Ok(DomainMutationOutcome::Created(record))
    }

    pub async fn remove(
        &self,
        actor: DomainActor<'_>,
        domain_id: &str,
        confirm_hostname: &str,
    ) -> Result<DomainMutationOutcome> {
        let Some((record, role)) = self.get_with_role(actor, domain_id).await? else {
            return Ok(DomainMutationOutcome::Missing);
        };
        if !actor.is_admin && !role.can_manage_services() {
            return Ok(DomainMutationOutcome::Forbidden);
        }
        if record.hostname.as_str() != confirm_hostname {
            return Err(DatabaseError::DomainConfirmationMismatch);
        }
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let deleted = sqlx::query("DELETE FROM domains WHERE id = ?")
            .bind(domain_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        if deleted == 0 {
            tx.commit().await?;
            return Ok(DomainMutationOutcome::Missing);
        }
        insert_audit(
            &mut tx,
            actor.id,
            "domain.remove_requested",
            domain_id,
            &now,
        )
        .await?;
        tx.commit().await?;
        Ok(DomainMutationOutcome::Removed(record))
    }

    pub async fn active_for_service(&self, service_id: &str) -> Result<Vec<DomainRecord>> {
        let rows = sqlx::query_as::<_, DomainRow>(
            "SELECT id, service_id, hostname, status, last_error,
                    dns_record_type, dns_record_target, dns_status, dns_error, dns_checked_at,
                    created_at, updated_at
             FROM domains WHERE service_id = ? ORDER BY created_at",
        )
        .bind(service_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(domain_from_row).collect()
    }

    pub async fn all(&self) -> Result<Vec<DomainRecord>> {
        let rows = sqlx::query_as::<_, DomainRow>(
            "SELECT id, service_id, hostname, status, last_error,
                    dns_record_type, dns_record_target, dns_status, dns_error, dns_checked_at,
                    created_at, updated_at
             FROM domains ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(domain_from_row).collect()
    }

    pub async fn set_status(
        &self,
        domain_id: &str,
        status: DomainStatus,
        last_error: Option<&str>,
    ) -> Result<()> {
        sqlx::query("UPDATE domains SET status = ?, last_error = ?, updated_at = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(last_error)
            .bind(Utc::now().to_rfc3339())
            .bind(domain_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn request_dns_verification(
        &self,
        actor: DomainActor<'_>,
        domain_id: &str,
    ) -> Result<DomainVerificationRequestOutcome> {
        let Some((record, role)) = self.get_with_role(actor, domain_id).await? else {
            return Ok(DomainVerificationRequestOutcome::Missing);
        };
        if !actor.is_admin && !role.can_manage_services() {
            return Ok(DomainVerificationRequestOutcome::Forbidden);
        }
        if record.dns_record.is_none() {
            return Ok(DomainVerificationRequestOutcome::NotConfigured);
        }
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE domains
             SET dns_status = 'pending', dns_error = NULL, dns_verification_requested_at = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&now)
        .bind(&now)
        .bind(domain_id)
        .execute(&mut *tx)
        .await?;
        insert_audit(
            &mut tx,
            actor.id,
            "domain.dns_verification_requested",
            domain_id,
            &now,
        )
        .await?;
        let updated = fetch_domain(&mut tx, domain_id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
        tx.commit().await?;
        Ok(DomainVerificationRequestOutcome::Requested(Box::new(
            updated,
        )))
    }

    pub async fn pending_dns_verifications(&self) -> Result<Vec<DomainRecord>> {
        let rows = sqlx::query_as::<_, DomainRow>(
            "SELECT id, service_id, hostname, status, last_error,
                    dns_record_type, dns_record_target, dns_status, dns_error, dns_checked_at,
                    created_at, updated_at
             FROM domains
             WHERE dns_verification_requested_at IS NOT NULL
             ORDER BY dns_verification_requested_at",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(domain_from_row).collect()
    }

    pub async fn complete_dns_verification(
        &self,
        domain_id: &str,
        status: DnsVerificationStatus,
        error: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE domains
             SET dns_status = ?, dns_error = ?, dns_checked_at = ?,
                 dns_verification_requested_at = NULL, updated_at = ?
             WHERE id = ?",
        )
        .bind(status.as_str())
        .bind(error)
        .bind(&now)
        .bind(&now)
        .bind(domain_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_with_role(
        &self,
        actor: DomainActor<'_>,
        domain_id: &str,
    ) -> Result<Option<(DomainRecord, ProjectMemberRole)>> {
        let row = sqlx::query_as::<_, DomainWithProjectRow>(
            "SELECT d.id, d.service_id, d.hostname, d.status, d.last_error,
                    d.dns_record_type, d.dns_record_target, d.dns_status, d.dns_error, d.dns_checked_at,
                    d.created_at, d.updated_at,
                    e.project_id
             FROM domains d
             JOIN services s ON s.id = d.service_id
             JOIN environments e ON e.id = s.environment_id
             WHERE d.id = ?",
        )
        .bind(domain_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let Some(role) = self.project_role(actor, &row.project_id).await? else {
            return Ok(None);
        };
        Ok(Some((domain_from_row(row.into())?, role)))
    }

    async fn service_role(
        &self,
        actor: DomainActor<'_>,
        service_id: &str,
    ) -> Result<Option<ProjectMemberRole>> {
        let project_id: Option<String> = sqlx::query_scalar(
            "SELECT e.project_id FROM services s JOIN environments e ON e.id = s.environment_id
             WHERE s.id = ?",
        )
        .bind(service_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(project_id) = project_id else {
            return Ok(None);
        };
        self.project_role(actor, &project_id).await
    }

    async fn project_role(
        &self,
        actor: DomainActor<'_>,
        project_id: &str,
    ) -> Result<Option<ProjectMemberRole>> {
        if actor.is_admin {
            let exists: Option<String> = sqlx::query_scalar("SELECT id FROM projects WHERE id = ?")
                .bind(project_id)
                .fetch_optional(&self.pool)
                .await?;
            return Ok(exists.map(|_| ProjectMemberRole::Owner));
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

async fn fetch_domain(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    domain_id: &str,
) -> Result<Option<DomainRecord>> {
    let row = sqlx::query_as::<_, DomainRow>(
        "SELECT id, service_id, hostname, status, last_error,
                dns_record_type, dns_record_target, dns_status, dns_error, dns_checked_at,
                created_at, updated_at
         FROM domains WHERE id = ?",
    )
    .bind(domain_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(domain_from_row).transpose()
}

async fn insert_audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    actor_id: &str,
    action: &str,
    domain_id: &str,
    now: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO audit_logs (id, user_id, action, resource_type, resource_id, created_at)
         VALUES (?, ?, ?, 'domain', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(actor_id)
    .bind(action)
    .bind(domain_id)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn domain_from_row(row: DomainRow) -> Result<DomainRecord> {
    let dns_status = row
        .dns_status
        .as_str()
        .try_into()
        .map_err(|_| DatabaseError::InvalidDnsVerificationStatus(row.dns_status.clone()))?;
    let dns_record = if row.dns_record_target.trim().is_empty() {
        None
    } else {
        let record_type = row
            .dns_record_type
            .as_str()
            .try_into()
            .map_err(|_| DatabaseError::InvalidDnsRecordType(row.dns_record_type.clone()))?;
        Some(
            DnsRecord::new(record_type, &row.dns_record_target).map_err(|_| {
                DatabaseError::InvalidDnsRecordTarget(row.dns_record_target.clone())
            })?,
        )
    };
    Ok(DomainRecord {
        id: DomainId::new(row.id)
            .map_err(|_| sqlx::Error::Protocol("stored domain id is invalid".into()))?,
        service_id: ServiceId::new(row.service_id)
            .map_err(|_| sqlx::Error::Protocol("stored service id is invalid".into()))?,
        hostname: DomainName::new(row.hostname)
            .map_err(|_| sqlx::Error::Protocol("stored domain name is invalid".into()))?,
        status: row
            .status
            .as_str()
            .try_into()
            .map_err(|_| DatabaseError::InvalidDomainStatus(row.status))?,
        last_error: row.last_error,
        dns_record,
        dns_status,
        dns_error: row.dns_error,
        dns_checked_at: row.dns_checked_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

#[derive(Debug, FromRow)]
struct DomainRow {
    id: String,
    service_id: String,
    hostname: String,
    status: String,
    last_error: Option<String>,
    dns_record_type: String,
    dns_record_target: String,
    dns_status: String,
    dns_error: Option<String>,
    dns_checked_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct DomainWithProjectRow {
    id: String,
    service_id: String,
    hostname: String,
    status: String,
    last_error: Option<String>,
    dns_record_type: String,
    dns_record_target: String,
    dns_status: String,
    dns_error: Option<String>,
    dns_checked_at: Option<String>,
    created_at: String,
    updated_at: String,
    project_id: String,
}

impl From<DomainWithProjectRow> for DomainRow {
    fn from(row: DomainWithProjectRow) -> Self {
        Self {
            id: row.id,
            service_id: row.service_id,
            hostname: row.hostname,
            status: row.status,
            last_error: row.last_error,
            dns_record_type: row.dns_record_type,
            dns_record_target: row.dns_record_target,
            dns_status: row.dns_status,
            dns_error: row.dns_error,
            dns_checked_at: row.dns_checked_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}
