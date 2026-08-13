use sqlx::{FromRow, SqlitePool};

use crate::Result;

#[derive(Debug, Clone)]
pub struct OperationsSummary {
    pub deployments: DeploymentOperationsSummary,
    pub backup: BackupOperationsSummary,
    pub domains: DomainOperationsSummary,
    pub certificates: CertificateOperationsSummary,
    pub remote_agents: RemoteAgentOperationsSummary,
}

#[derive(Debug, Clone)]
pub struct DeploymentOperationsSummary {
    pub queued_count: i64,
    pub active_count: i64,
    pub failed_count: i64,
    pub failed_retry_count: i64,
    pub retry_count: i64,
    pub average_duration_seconds: Option<f64>,
    pub latest_duration_seconds: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct BackupOperationsSummary {
    pub configured: bool,
    pub enabled: bool,
    pub schedule_interval_hours: Option<u16>,
    pub latest_scheduled_run: Option<BackupRunSummary>,
}

#[derive(Debug, Clone)]
pub struct BackupRunSummary {
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DomainOperationsSummary {
    pub active_count: i64,
    pub pending_count: i64,
    pub failed_count: i64,
}

#[derive(Debug, Clone)]
pub struct CertificateOperationsSummary {
    pub https_enabled: bool,
    pub provider: String,
    pub custom_certificate_selected: bool,
    pub stored_certificate_count: i64,
}

#[derive(Debug, Clone)]
pub struct RemoteAgentOperationsSummary {
    pub server_count: i64,
    pub online_count: i64,
    pub offline_count: i64,
    pub pending_count: i64,
    pub oldest_heartbeat_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OperationsRepository {
    pool: SqlitePool,
}

impl OperationsRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn summary(&self) -> Result<OperationsSummary> {
        let deployment = sqlx::query_as::<_, DeploymentOperationsRow>(
            "SELECT
                COALESCE(SUM(CASE WHEN status = 'queued' THEN 1 ELSE 0 END), 0) AS queued_count,
                COALESCE(SUM(CASE WHEN status IN ('queued', 'preparing', 'running', 'stopping') THEN 1 ELSE 0 END), 0) AS active_count,
                COALESCE(SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END), 0) AS failed_count,
                COALESCE(SUM(CASE WHEN status = 'failed' AND attempt_count > 1 THEN 1 ELSE 0 END), 0) AS failed_retry_count,
                COALESCE(SUM(CASE WHEN attempt_count > 1 THEN attempt_count - 1 ELSE 0 END), 0)
                    + COALESCE(SUM(CASE WHEN retry_after IS NOT NULL THEN 1 ELSE 0 END), 0) AS retry_count,
                (SELECT AVG((julianday(finished_at) - julianday(started_at)) * 86400.0)
                 FROM (
                    SELECT started_at, finished_at
                    FROM deployments
                    WHERE started_at IS NOT NULL AND finished_at IS NOT NULL
                    ORDER BY finished_at DESC
                    LIMIT 100
                 )) AS average_duration_seconds
             FROM deployments",
        )
        .fetch_one(&self.pool)
        .await?;
        let latest_duration_seconds = sqlx::query_scalar::<_, Option<f64>>(
            "SELECT CASE WHEN started_at IS NOT NULL AND finished_at IS NOT NULL
                    THEN (julianday(finished_at) - julianday(started_at)) * 86400.0
                END
             FROM deployments
             WHERE started_at IS NOT NULL AND finished_at IS NOT NULL
             ORDER BY finished_at DESC
             LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?
        .flatten();

        let destination = sqlx::query_as::<_, BackupDestinationRow>(
            "SELECT enabled, schedule_interval_hours
             FROM backup_s3_destination
             WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        let latest_scheduled_run = sqlx::query_as::<_, BackupRunRow>(
            "SELECT status, started_at, completed_at
             FROM backup_s3_run
             WHERE trigger = 'scheduled'
             ORDER BY started_at DESC
             LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?
        .map(BackupRunRow::into_summary);

        let domains = sqlx::query_as::<_, DomainOperationsRow>(
            "SELECT
                COALESCE(SUM(CASE WHEN status = 'active' THEN 1 ELSE 0 END), 0) AS active_count,
                COALESCE(SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END), 0) AS pending_count,
                COALESCE(SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END), 0) AS failed_count
             FROM domains",
        )
        .fetch_one(&self.pool)
        .await?;

        let certificates = sqlx::query_as::<_, CertificateOperationsRow>(
            "SELECT https_enabled, certificate_provider,
                    custom_certificate_id IS NOT NULL AS custom_certificate_selected,
                    (SELECT COUNT(*) FROM server_certificates) AS stored_certificate_count
             FROM server_settings
             WHERE id = 1",
        )
        .fetch_one(&self.pool)
        .await?;

        let remote_agents = sqlx::query_as::<_, RemoteAgentOperationsRow>(
            "SELECT COUNT(rs.id) AS server_count,
                    COALESCE(SUM(CASE WHEN a.status = 'online' THEN 1 ELSE 0 END), 0) AS online_count,
                    COALESCE(SUM(CASE WHEN a.status = 'offline' THEN 1 ELSE 0 END), 0) AS offline_count,
                    COALESCE(SUM(CASE WHEN a.status IS NULL OR a.status = 'pending' THEN 1 ELSE 0 END), 0) AS pending_count,
                    MIN(a.last_heartbeat_at) AS oldest_heartbeat_at
             FROM remote_servers rs
             LEFT JOIN remote_server_agents a ON a.server_id = rs.id",
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(OperationsSummary {
            deployments: DeploymentOperationsSummary {
                queued_count: deployment.queued_count,
                active_count: deployment.active_count,
                failed_count: deployment.failed_count,
                failed_retry_count: deployment.failed_retry_count,
                retry_count: deployment.retry_count,
                average_duration_seconds: deployment.average_duration_seconds,
                latest_duration_seconds,
            },
            backup: BackupOperationsSummary {
                configured: destination.is_some(),
                enabled: destination.as_ref().is_some_and(|value| value.enabled),
                schedule_interval_hours: destination
                    .and_then(|value| value.schedule_interval_hours),
                latest_scheduled_run,
            },
            domains: DomainOperationsSummary {
                active_count: domains.active_count,
                pending_count: domains.pending_count,
                failed_count: domains.failed_count,
            },
            certificates: CertificateOperationsSummary {
                https_enabled: certificates.https_enabled,
                provider: certificates.certificate_provider,
                custom_certificate_selected: certificates.custom_certificate_selected,
                stored_certificate_count: certificates.stored_certificate_count,
            },
            remote_agents: RemoteAgentOperationsSummary {
                server_count: remote_agents.server_count,
                online_count: remote_agents.online_count,
                offline_count: remote_agents.offline_count,
                pending_count: remote_agents.pending_count,
                oldest_heartbeat_at: remote_agents.oldest_heartbeat_at,
            },
        })
    }
}

#[derive(Debug, FromRow)]
struct DeploymentOperationsRow {
    queued_count: i64,
    active_count: i64,
    failed_count: i64,
    failed_retry_count: i64,
    retry_count: i64,
    average_duration_seconds: Option<f64>,
}

#[derive(Debug, FromRow)]
struct BackupDestinationRow {
    enabled: bool,
    schedule_interval_hours: Option<u16>,
}

#[derive(Debug, FromRow)]
struct BackupRunRow {
    status: String,
    started_at: String,
    completed_at: Option<String>,
}

impl BackupRunRow {
    fn into_summary(self) -> BackupRunSummary {
        BackupRunSummary {
            status: self.status,
            started_at: self.started_at,
            completed_at: self.completed_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct DomainOperationsRow {
    active_count: i64,
    pending_count: i64,
    failed_count: i64,
}

#[derive(Debug, FromRow)]
struct CertificateOperationsRow {
    https_enabled: bool,
    certificate_provider: String,
    custom_certificate_selected: bool,
    stored_certificate_count: i64,
}

#[derive(Debug, FromRow)]
struct RemoteAgentOperationsRow {
    server_count: i64,
    online_count: i64,
    offline_count: i64,
    pending_count: i64,
    oldest_heartbeat_at: Option<String>,
}
