use ignitify_domain::{
    DeploymentApproval, DeploymentId, DeploymentState, ProjectMemberRole, ServiceId,
    ServiceSourceConfig, ServiceSpec, SupplyChainReport,
};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{DatabaseError, Result};

#[path = "deployment_data.rs"]
mod deployment_data;
#[path = "deployment_lifecycle.rs"]
mod deployment_lifecycle;
#[path = "deployment_reads.rs"]
mod deployment_reads;
#[path = "deployment_retry.rs"]
mod deployment_retry;
#[path = "deployment_streams.rs"]
mod deployment_streams;
#[path = "supply_chain_policy.rs"]
mod supply_chain_policy;

use deployment_data::{
    DeploymentRow, DeploymentWithProjectRow, ProjectVariableRow, ServiceRow, VariableRow,
    decode_source_config, decode_spec, deployment_from_row, parse_service_id,
};

const HISTORY_PAGE_SIZE: i64 = 50;
const HISTORY_PAGE_MAX: i64 = 100;

#[derive(Debug, Clone, Copy)]
pub struct DeploymentActor<'a> {
    pub id: &'a str,
    pub is_admin: bool,
}

#[derive(Debug, Clone)]
pub struct AuthorizedDeploymentService {
    pub id: ServiceId,
    pub role: ProjectMemberRole,
    pub desired_generation: i64,
    pub spec: ServiceSpec,
    pub source_config: Option<ServiceSourceConfig>,
    pub deployment_destination_id: Option<String>,
    pub is_production: bool,
    pub variables: Vec<DeploymentVariableRecord>,
}

#[derive(Debug, Clone)]
pub struct DeploymentVariableRecord {
    pub key: String,
    pub ciphertext: String,
}

#[derive(Debug, Clone)]
pub struct NewDeployment {
    pub idempotency_key: String,
    pub requested_by_user_id: String,
    pub spec: ServiceSpec,
    pub source_config: Option<ServiceSourceConfig>,
    pub deployment_destination_id: Option<String>,
    pub source_revision: Option<String>,
    pub supply_chain_report: Option<SupplyChainReport>,
    pub variables_ciphertext: String,
}

#[derive(Debug, Clone)]
pub struct DeploymentRecord {
    pub id: DeploymentId,
    pub correlation_id: String,
    pub service_id: ServiceId,
    pub generation: i64,
    pub idempotency_key: String,
    pub requested_by_user_id: String,
    pub spec: ServiceSpec,
    pub source_config: Option<ServiceSourceConfig>,
    pub deployment_destination_id: Option<String>,
    pub source_revision: Option<String>,
    pub local_image_id: Option<String>,
    pub supply_chain_report: Option<SupplyChainReport>,
    pub approval: DeploymentApproval,
    pub variables_ciphertext: String,
    pub runtime_ref: Option<String>,
    pub state: DeploymentState,
    pub failure_reason: Option<String>,
    pub attempt_count: i64,
    pub retry_after: Option<String>,
    pub cancel_requested_at: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeploymentEventRecord {
    pub sequence: i64,
    pub event_id: String,
    pub deployment_id: DeploymentId,
    pub correlation_id: String,
    pub kind: String,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct DeploymentLogRecord {
    pub sequence: i64,
    pub deployment_id: DeploymentId,
    pub correlation_id: String,
    pub stream: String,
    pub line: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequenceCursor {
    pub oldest: Option<i64>,
    pub newest: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewDeploymentLog {
    pub stream: String,
    pub line: String,
}

#[derive(Debug, Clone)]
pub enum CreateDeploymentOutcome {
    Created(DeploymentRecord),
    Existing(DeploymentRecord),
    Missing,
    Forbidden,
    ActiveConflict,
}

#[derive(Debug, Clone)]
pub enum CancelDeploymentOutcome {
    Cancelled(DeploymentRecord),
    Existing(DeploymentRecord),
    Missing,
    Forbidden,
}

#[derive(Debug, Clone)]
pub enum DeploymentApprovalOutcome {
    Approved(DeploymentRecord),
    Existing(DeploymentRecord),
    Missing,
    Forbidden,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetrySchedule {
    Scheduled { retry_after: String },
    Exhausted,
    Cancelled,
    Unchanged,
}

#[derive(Debug, Clone)]
pub struct DeploymentsRepository {
    pool: SqlitePool,
}

impl DeploymentsRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    async fn project_role(
        &self,
        actor: DeploymentActor<'_>,
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

async fn next_generation(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    service_id: &str,
    desired_generation: i64,
) -> Result<i64> {
    let latest: Option<i64> =
        sqlx::query_scalar("SELECT MAX(generation) FROM deployments WHERE service_id = ?")
            .bind(service_id)
            .fetch_one(&mut **tx)
            .await?;
    Ok(latest.unwrap_or(0).max(desired_generation - 1) + 1)
}

async fn fetch_by_service_key(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    service_id: &str,
    idempotency_key: &str,
) -> Result<Option<DeploymentRecord>> {
    let row = sqlx::query_as::<_, DeploymentRow>(
        "SELECT id, correlation_id, service_id, generation, idempotency_key, requested_by_user_id, spec_json, runtime_spec_json,
                source_config_json, deployment_destination_id, source_revision, local_image_id, supply_chain_report_json,
                approval_status, approval_requested_at, approved_by_user_id, approved_at, variables_ciphertext, runtime_ref,
                status, failure_reason, attempt_count, retry_after, cancel_requested_at,
                created_at, started_at, finished_at
         FROM deployments WHERE service_id = ? AND idempotency_key = ?",
    )
    .bind(service_id)
    .bind(idempotency_key)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(deployment_from_row).transpose()
}

async fn fetch_deployment(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    deployment_id: &str,
) -> Result<Option<DeploymentRecord>> {
    let row = sqlx::query_as::<_, DeploymentRow>(
        "SELECT id, correlation_id, service_id, generation, idempotency_key, requested_by_user_id, spec_json, runtime_spec_json,
                source_config_json, deployment_destination_id, source_revision, local_image_id, supply_chain_report_json,
                approval_status, approval_requested_at, approved_by_user_id, approved_at, variables_ciphertext, runtime_ref,
                status, failure_reason, attempt_count, retry_after, cancel_requested_at,
                created_at, started_at, finished_at
         FROM deployments WHERE id = ?",
    )
    .bind(deployment_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(deployment_from_row).transpose()
}

async fn insert_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    deployment_id: &str,
    kind: &str,
    payload: serde_json::Value,
    now: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO deployment_events (deployment_id, correlation_id, kind, payload_json, created_at)
         SELECT id, correlation_id, ?, ?, ? FROM deployments WHERE id = ?",
    )
    .bind(kind)
    .bind(payload.to_string())
    .bind(now)
    .bind(deployment_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    actor_id: &str,
    action: &str,
    deployment_id: &str,
    correlation_id: &str,
    now: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO audit_logs (id, user_id, action, resource_type, resource_id, correlation_id, created_at)
         VALUES (?, ?, ?, 'deployment', ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(actor_id)
    .bind(action)
    .bind(deployment_id)
    .bind(correlation_id)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn history_limit(limit: Option<i64>) -> i64 {
    limit
        .unwrap_or(HISTORY_PAGE_SIZE)
        .clamp(1, HISTORY_PAGE_MAX)
}
