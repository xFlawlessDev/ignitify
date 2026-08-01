use chrono::Utc;
use ignitify_domain::{DeploymentId, DeploymentState, ProjectMemberRole, ServiceId, ServiceSpec};
use serde_json::json;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::{DatabaseError, Result};

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
    pub variables_ciphertext: String,
}

#[derive(Debug, Clone)]
pub struct DeploymentRecord {
    pub id: DeploymentId,
    pub service_id: ServiceId,
    pub generation: i64,
    pub idempotency_key: String,
    pub requested_by_user_id: String,
    pub spec: ServiceSpec,
    pub variables_ciphertext: String,
    pub runtime_ref: Option<String>,
    pub state: DeploymentState,
    pub failure_reason: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeploymentEventRecord {
    pub sequence: i64,
    pub deployment_id: DeploymentId,
    pub kind: String,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct DeploymentLogRecord {
    pub sequence: i64,
    pub deployment_id: DeploymentId,
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
pub struct DeploymentsRepository {
    pool: SqlitePool,
}

impl DeploymentsRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn service_for_deployment(
        &self,
        actor: DeploymentActor<'_>,
        service_id: &str,
    ) -> Result<Option<AuthorizedDeploymentService>> {
        let row = sqlx::query_as::<_, ServiceRow>(
            "SELECT s.id, e.project_id, s.desired_generation, s.desired_spec_json
             FROM services s JOIN environments e ON e.id = s.environment_id
             WHERE s.id = ?",
        )
        .bind(service_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let Some(role) = self.project_role(actor, &row.project_id).await? else {
            return Ok(None);
        };
        let spec = decode_spec(&row.desired_spec_json)?;
        let variables = sqlx::query_as::<_, VariableRow>(
            "SELECT key, ciphertext FROM service_variables WHERE service_id = ? ORDER BY key",
        )
        .bind(&row.id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| DeploymentVariableRecord {
            key: row.key,
            ciphertext: row.ciphertext,
        })
        .collect();
        Ok(Some(AuthorizedDeploymentService {
            id: parse_service_id(row.id)?,
            role,
            desired_generation: row.desired_generation,
            spec,
            variables,
        }))
    }

    pub async fn create(
        &self,
        actor: DeploymentActor<'_>,
        service_id: &str,
        deployment: NewDeployment,
    ) -> Result<CreateDeploymentOutcome> {
        let Some(service) = self.service_for_deployment(actor, service_id).await? else {
            return Ok(CreateDeploymentOutcome::Missing);
        };
        if !actor.is_admin && !service.role.can_manage_services() {
            return Ok(CreateDeploymentOutcome::Forbidden);
        }
        let mut tx = self.pool.begin().await?;
        if let Some(existing) =
            fetch_by_service_key(&mut tx, service.id.as_str(), &deployment.idempotency_key).await?
        {
            return Ok(CreateDeploymentOutcome::Existing(existing));
        }
        let active: Option<String> = sqlx::query_scalar(
            "SELECT id FROM deployments
             WHERE service_id = ? AND status IN ('queued', 'preparing', 'running')",
        )
        .bind(service.id.as_str())
        .fetch_optional(&mut *tx)
        .await?;
        if active.is_some() {
            return Ok(CreateDeploymentOutcome::ActiveConflict);
        }
        let generation =
            next_generation(&mut tx, service.id.as_str(), service.desired_generation).await?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let spec_json = serde_json::to_string(&deployment.spec)
            .map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
        let inserted = sqlx::query(
            "INSERT INTO deployments (
                id, service_id, generation, idempotency_key, requested_by_user_id, spec_json,
                variables_ciphertext, status, created_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, 'queued', ?)
             ON CONFLICT(service_id, idempotency_key) DO NOTHING
             ON CONFLICT(service_id) WHERE status IN ('queued', 'preparing', 'running') DO NOTHING",
        )
        .bind(&id)
        .bind(service.id.as_str())
        .bind(generation)
        .bind(&deployment.idempotency_key)
        .bind(&deployment.requested_by_user_id)
        .bind(spec_json)
        .bind(&deployment.variables_ciphertext)
        .bind(&now)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if inserted == 0 {
            if let Some(existing) =
                fetch_by_service_key(&mut tx, service.id.as_str(), &deployment.idempotency_key)
                    .await?
            {
                tx.commit().await?;
                return Ok(CreateDeploymentOutcome::Existing(existing));
            }
            tx.commit().await?;
            return Ok(CreateDeploymentOutcome::ActiveConflict);
        }
        insert_event(&mut tx, &id, "deployment.queued", json!({}), &now).await?;
        insert_audit(
            &mut tx,
            &deployment.requested_by_user_id,
            "deployment.create",
            &id,
            &now,
        )
        .await?;
        let record = fetch_deployment(&mut tx, &id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
        tx.commit().await?;
        Ok(CreateDeploymentOutcome::Created(record))
    }

    pub async fn get(
        &self,
        actor: DeploymentActor<'_>,
        deployment_id: &str,
    ) -> Result<Option<DeploymentRecord>> {
        let row = sqlx::query_as::<_, DeploymentWithProjectRow>(
            "SELECT d.id, d.service_id, d.generation, d.idempotency_key, d.requested_by_user_id,
                    d.spec_json, d.variables_ciphertext, d.runtime_ref, d.status, d.failure_reason,
                    d.created_at, d.started_at, d.finished_at, e.project_id
             FROM deployments d
             JOIN services s ON s.id = d.service_id
             JOIN environments e ON e.id = s.environment_id
             WHERE d.id = ?",
        )
        .bind(deployment_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        if self.project_role(actor, &row.project_id).await?.is_none() {
            return Ok(None);
        }
        Ok(Some(deployment_from_row(row.into())?))
    }

    pub async fn list(
        &self,
        actor: DeploymentActor<'_>,
        service_id: &str,
        before_created_at: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Option<Vec<DeploymentRecord>>> {
        if self
            .service_for_deployment(actor, service_id)
            .await?
            .is_none()
        {
            return Ok(None);
        }
        let rows = sqlx::query_as::<_, DeploymentRow>(
            "SELECT id, service_id, generation, idempotency_key, requested_by_user_id, spec_json,
                    variables_ciphertext, runtime_ref, status, failure_reason, created_at, started_at, finished_at
             FROM deployments
             WHERE service_id = ? AND (? IS NULL OR created_at < ?)
             ORDER BY created_at DESC LIMIT ?",
        )
        .bind(service_id)
        .bind(before_created_at)
        .bind(before_created_at)
        .bind(history_limit(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(deployment_from_row)
            .collect::<Result<Vec<_>>>()
            .map(Some)
    }

    pub async fn list_for_project(
        &self,
        actor: DeploymentActor<'_>,
        project_id: &str,
        before_created_at: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Option<Vec<DeploymentRecord>>> {
        if self.project_role(actor, project_id).await?.is_none() {
            return Ok(None);
        }
        let rows = sqlx::query_as::<_, DeploymentRow>(
            "SELECT d.id, d.service_id, d.generation, d.idempotency_key, d.requested_by_user_id,
                    d.spec_json, d.variables_ciphertext, d.runtime_ref, d.status, d.failure_reason,
                    d.created_at, d.started_at, d.finished_at
             FROM deployments d
             JOIN services s ON s.id = d.service_id
             JOIN environments e ON e.id = s.environment_id
             WHERE e.project_id = ? AND (? IS NULL OR d.created_at < ?)
             ORDER BY d.created_at DESC LIMIT ?",
        )
        .bind(project_id)
        .bind(before_created_at)
        .bind(before_created_at)
        .bind(history_limit(limit))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(deployment_from_row)
            .collect::<Result<Vec<_>>>()
            .map(Some)
    }

    pub async fn active_for_stop(
        &self,
        actor: DeploymentActor<'_>,
        service_id: &str,
    ) -> Result<Option<DeploymentRecord>> {
        if self
            .service_for_deployment(actor, service_id)
            .await?
            .is_none()
        {
            return Ok(None);
        }
        let row = sqlx::query_as::<_, DeploymentRow>(
            "SELECT id, service_id, generation, idempotency_key, requested_by_user_id, spec_json,
                    variables_ciphertext, runtime_ref, status, failure_reason, created_at, started_at, finished_at
             FROM deployments
             WHERE service_id = ? AND status IN ('running', 'healthy')
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(service_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(deployment_from_row).transpose()
    }

    pub async fn claim_next(&self) -> Result<Option<DeploymentRecord>> {
        let mut tx = self.pool.begin().await?;
        let id: Option<String> = sqlx::query_scalar(
            "SELECT id FROM deployments WHERE status = 'queued' ORDER BY created_at LIMIT 1",
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(id) = id else {
            tx.commit().await?;
            return Ok(None);
        };
        let now = Utc::now().to_rfc3339();
        let changed = sqlx::query(
            "UPDATE deployments SET status = 'preparing', started_at = ? WHERE id = ? AND status = 'queued'",
        )
        .bind(&now)
        .bind(&id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if changed == 0 {
            tx.commit().await?;
            return Ok(None);
        }
        insert_event(&mut tx, &id, "deployment.preparing", json!({}), &now).await?;
        let record = fetch_deployment(&mut tx, &id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
        tx.commit().await?;
        Ok(Some(record))
    }

    pub async fn reset_preparing_without_runtime(&self, deployment_id: &str) -> Result<bool> {
        let changed = sqlx::query(
            "UPDATE deployments SET status = 'queued', started_at = NULL
             WHERE id = ? AND status = 'preparing' AND runtime_ref IS NULL",
        )
        .bind(deployment_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(changed == 1)
    }

    pub async fn record_runtime_ref(&self, deployment_id: &str, runtime_ref: &str) -> Result<bool> {
        let changed = sqlx::query(
            "UPDATE deployments SET runtime_ref = ?
             WHERE id = ? AND status = 'preparing' AND runtime_ref IS NULL",
        )
        .bind(runtime_ref)
        .bind(deployment_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(changed == 1)
    }

    pub async fn replace_runtime_ref(
        &self,
        deployment_id: &str,
        runtime_ref: &str,
    ) -> Result<bool> {
        let changed = sqlx::query(
            "UPDATE deployments SET runtime_ref = ? WHERE id = ? AND status = 'preparing'",
        )
        .bind(runtime_ref)
        .bind(deployment_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(changed == 1)
    }

    pub async fn healthy_prior_deployments(
        &self,
        service_id: &str,
        deployment_id: &str,
    ) -> Result<Vec<DeploymentRecord>> {
        let rows = sqlx::query_as::<_, DeploymentRow>(
            "SELECT id, service_id, generation, idempotency_key, requested_by_user_id, spec_json,
                    variables_ciphertext, runtime_ref, status, failure_reason, created_at, started_at, finished_at
             FROM deployments
             WHERE service_id = ? AND id != ? AND status = 'healthy'",
        )
        .bind(service_id)
        .bind(deployment_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(deployment_from_row).collect()
    }

    pub async fn supersede_prior_healthy(
        &self,
        service_id: &str,
        deployment_id: &str,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let rows = sqlx::query_scalar::<_, String>(
            "SELECT id FROM deployments
             WHERE service_id = ? AND id != ? AND status = 'healthy'",
        )
        .bind(service_id)
        .bind(deployment_id)
        .fetch_all(&mut *tx)
        .await?;
        for prior_id in rows {
            sqlx::query(
                "UPDATE deployments SET status = 'superseded', finished_at = ? WHERE id = ? AND status = 'healthy'",
            )
            .bind(&now)
            .bind(&prior_id)
            .execute(&mut *tx)
            .await?;
            insert_event(
                &mut tx,
                &prior_id,
                "deployment.superseded",
                json!({ "successor_id": deployment_id }),
                &now,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn transition(
        &self,
        deployment_id: &str,
        next: DeploymentState,
        runtime_ref: Option<&str>,
        failure_reason: Option<&str>,
    ) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let current: Option<String> =
            sqlx::query_scalar("SELECT status FROM deployments WHERE id = ?")
                .bind(deployment_id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some(current) = current else {
            tx.commit().await?;
            return Ok(false);
        };
        let current = DeploymentState::try_from(current.as_str())
            .map_err(|_| DatabaseError::InvalidDeploymentState(current))?;
        if !current.can_transition_to(next) {
            tx.commit().await?;
            return Ok(false);
        }
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE deployments
             SET status = ?, runtime_ref = COALESCE(?, runtime_ref), failure_reason = ?,
                 finished_at = CASE WHEN ? THEN ? ELSE finished_at END
             WHERE id = ?",
        )
        .bind(next.as_str())
        .bind(runtime_ref)
        .bind(failure_reason)
        .bind(next.is_terminal())
        .bind(&now)
        .bind(deployment_id)
        .execute(&mut *tx)
        .await?;
        insert_event(
            &mut tx,
            deployment_id,
            &format!("deployment.{}", next.as_str()),
            json!({ "failure_reason": failure_reason }),
            &now,
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn routable(&self) -> Result<Vec<DeploymentRecord>> {
        let rows = sqlx::query_as::<_, DeploymentRow>(
            "SELECT id, service_id, generation, idempotency_key, requested_by_user_id, spec_json,
                    variables_ciphertext, runtime_ref, status, failure_reason, created_at, started_at, finished_at
             FROM deployments WHERE status IN ('running', 'healthy') AND runtime_ref IS NOT NULL",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(deployment_from_row).collect()
    }

    pub async fn nonterminal(&self) -> Result<Vec<DeploymentRecord>> {
        let rows = sqlx::query_as::<_, DeploymentRow>(
            "SELECT id, service_id, generation, idempotency_key, requested_by_user_id, spec_json,
                    variables_ciphertext, runtime_ref, status, failure_reason, created_at, started_at, finished_at
             FROM deployments
             WHERE status IN ('queued', 'preparing', 'running', 'stopping')
             ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(deployment_from_row).collect()
    }

    pub async fn latest_log_since(&self, deployment_id: &str) -> Result<Option<i64>> {
        Ok(sqlx::query_scalar(
            "SELECT CAST(strftime('%s', MAX(created_at)) AS INTEGER)
             FROM deployment_logs WHERE deployment_id = ?",
        )
        .bind(deployment_id)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn append_logs(
        &self,
        deployment_id: &str,
        logs: &[NewDeploymentLog],
    ) -> Result<Vec<DeploymentLogRecord>> {
        if logs.is_empty() {
            return Ok(Vec::new());
        }
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let mut inserted = Vec::with_capacity(logs.len());
        for log in logs {
            let sequence: i64 = sqlx::query_scalar(
                "INSERT INTO deployment_logs (deployment_id, stream, line, created_at)
                 VALUES (?, ?, ?, ?) RETURNING sequence",
            )
            .bind(deployment_id)
            .bind(&log.stream)
            .bind(bound_log_line(&log.line))
            .bind(&now)
            .fetch_one(&mut *tx)
            .await?;
            inserted.push(DeploymentLogRecord {
                sequence,
                deployment_id: DeploymentId::new(deployment_id)
                    .map_err(|_| sqlx::Error::Protocol("stored deployment id is invalid".into()))?,
                stream: log.stream.clone(),
                line: bound_log_line(&log.line),
                created_at: now.clone(),
            });
        }
        sqlx::query(
            "DELETE FROM deployment_logs
             WHERE deployment_id = ? AND sequence <= COALESCE(
                (SELECT sequence FROM deployment_logs
                 WHERE deployment_id = ?
                 ORDER BY sequence DESC LIMIT 1 OFFSET 10000),
                -1
             )",
        )
        .bind(deployment_id)
        .bind(deployment_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(inserted)
    }

    pub async fn prune_retention(&self) -> Result<()> {
        let cutoff = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM deployment_events
             WHERE deployment_id IN (
                SELECT id FROM deployments
                WHERE status IN ('healthy', 'failed', 'stopped', 'superseded')
                  AND finished_at < ?
             )",
        )
        .bind(&cutoff)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "DELETE FROM deployment_logs
             WHERE deployment_id IN (
                SELECT id FROM deployments
                WHERE status IN ('healthy', 'failed', 'stopped', 'superseded')
                  AND finished_at < ?
             )",
        )
        .bind(cutoff)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn events(&self, deployment_id: &str) -> Result<Vec<DeploymentEventRecord>> {
        self.events_after(deployment_id, 0, i64::MAX).await
    }

    pub async fn events_after(
        &self,
        deployment_id: &str,
        after: i64,
        through: i64,
    ) -> Result<Vec<DeploymentEventRecord>> {
        let rows = sqlx::query_as::<_, EventRow>(
            "SELECT sequence, deployment_id, kind, payload_json, created_at
             FROM deployment_events
             WHERE deployment_id = ? AND sequence > ? AND sequence <= ?
             ORDER BY sequence",
        )
        .bind(deployment_id)
        .bind(after)
        .bind(through)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(DeploymentEventRecord {
                    sequence: row.sequence,
                    deployment_id: DeploymentId::new(row.deployment_id).map_err(|_| {
                        sqlx::Error::Protocol("stored deployment id is invalid".into())
                    })?,
                    kind: row.kind,
                    payload_json: row.payload_json,
                    created_at: row.created_at,
                })
            })
            .collect()
    }

    pub async fn event_cursor(&self, deployment_id: &str) -> Result<SequenceCursor> {
        cursor(&self.pool, "deployment_events", deployment_id).await
    }

    pub async fn logs_after(
        &self,
        deployment_id: &str,
        after: i64,
        through: i64,
    ) -> Result<Vec<DeploymentLogRecord>> {
        let rows = sqlx::query_as::<_, LogRow>(
            "SELECT sequence, deployment_id, stream, line, created_at
             FROM deployment_logs
             WHERE deployment_id = ? AND sequence > ? AND sequence <= ?
             ORDER BY sequence",
        )
        .bind(deployment_id)
        .bind(after)
        .bind(through)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(DeploymentLogRecord {
                    sequence: row.sequence,
                    deployment_id: DeploymentId::new(row.deployment_id).map_err(|_| {
                        sqlx::Error::Protocol("stored deployment id is invalid".into())
                    })?,
                    stream: row.stream,
                    line: row.line,
                    created_at: row.created_at,
                })
            })
            .collect()
    }

    pub async fn log_cursor(&self, deployment_id: &str) -> Result<SequenceCursor> {
        cursor(&self.pool, "deployment_logs", deployment_id).await
    }

    pub async fn rollback(
        &self,
        actor: DeploymentActor<'_>,
        deployment_id: &str,
        idempotency_key: &str,
    ) -> Result<CreateDeploymentOutcome> {
        let Some(source) = self.get(actor, deployment_id).await? else {
            return Ok(CreateDeploymentOutcome::Missing);
        };
        let Some(service) = self
            .service_for_deployment(actor, source.service_id.as_str())
            .await?
        else {
            return Ok(CreateDeploymentOutcome::Missing);
        };
        if !actor.is_admin && !service.role.can_manage_services() {
            return Ok(CreateDeploymentOutcome::Forbidden);
        }
        self.create(
            actor,
            source.service_id.as_str(),
            NewDeployment {
                idempotency_key: idempotency_key.to_owned(),
                requested_by_user_id: actor.id.to_owned(),
                spec: source.spec,
                variables_ciphertext: source.variables_ciphertext,
            },
        )
        .await
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
        "SELECT id, service_id, generation, idempotency_key, requested_by_user_id, spec_json,
                variables_ciphertext, runtime_ref, status, failure_reason, created_at, started_at, finished_at
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
        "SELECT id, service_id, generation, idempotency_key, requested_by_user_id, spec_json,
                variables_ciphertext, runtime_ref, status, failure_reason, created_at, started_at, finished_at
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
        "INSERT INTO deployment_events (deployment_id, kind, payload_json, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(deployment_id)
    .bind(kind)
    .bind(payload.to_string())
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    actor_id: &str,
    action: &str,
    deployment_id: &str,
    now: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO audit_logs (id, user_id, action, resource_type, resource_id, created_at)
         VALUES (?, ?, ?, 'deployment', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(actor_id)
    .bind(action)
    .bind(deployment_id)
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

fn bound_log_line(line: &str) -> String {
    let mut end = line.len().min(16 * 1024);
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    line[..end].to_owned()
}

fn decode_spec(value: &str) -> Result<ServiceSpec> {
    let spec: ServiceSpec = serde_json::from_str(value)
        .map_err(|error| DatabaseError::InvalidServiceSpec(error.to_string()))?;
    spec.validate()
        .map_err(|error| DatabaseError::InvalidServiceSpec(error.to_string()))?;
    Ok(spec)
}

fn deployment_from_row(row: DeploymentRow) -> Result<DeploymentRecord> {
    Ok(DeploymentRecord {
        id: DeploymentId::new(row.id)
            .map_err(|_| sqlx::Error::Protocol("stored deployment id is invalid".into()))?,
        service_id: parse_service_id(row.service_id)?,
        generation: row.generation,
        idempotency_key: row.idempotency_key,
        requested_by_user_id: row.requested_by_user_id,
        spec: decode_spec(&row.spec_json)?,
        variables_ciphertext: row.variables_ciphertext,
        runtime_ref: row.runtime_ref,
        state: DeploymentState::try_from(row.status.as_str())
            .map_err(|_| DatabaseError::InvalidDeploymentState(row.status))?,
        failure_reason: row.failure_reason,
        created_at: row.created_at,
        started_at: row.started_at,
        finished_at: row.finished_at,
    })
}

fn parse_service_id(value: String) -> Result<ServiceId> {
    ServiceId::new(value)
        .map_err(|_| sqlx::Error::Protocol("stored service id is invalid".into()).into())
}

#[derive(Debug, FromRow)]
struct ServiceRow {
    id: String,
    project_id: String,
    desired_generation: i64,
    desired_spec_json: String,
}

#[derive(Debug, FromRow)]
struct VariableRow {
    key: String,
    ciphertext: String,
}

#[derive(Debug, FromRow)]
struct DeploymentRow {
    id: String,
    service_id: String,
    generation: i64,
    idempotency_key: String,
    requested_by_user_id: String,
    spec_json: String,
    variables_ciphertext: String,
    runtime_ref: Option<String>,
    status: String,
    failure_reason: Option<String>,
    created_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
}

#[derive(Debug, FromRow)]
struct DeploymentWithProjectRow {
    id: String,
    service_id: String,
    generation: i64,
    idempotency_key: String,
    requested_by_user_id: String,
    spec_json: String,
    variables_ciphertext: String,
    runtime_ref: Option<String>,
    status: String,
    failure_reason: Option<String>,
    created_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    project_id: String,
}

impl From<DeploymentWithProjectRow> for DeploymentRow {
    fn from(row: DeploymentWithProjectRow) -> Self {
        Self {
            id: row.id,
            service_id: row.service_id,
            generation: row.generation,
            idempotency_key: row.idempotency_key,
            requested_by_user_id: row.requested_by_user_id,
            spec_json: row.spec_json,
            variables_ciphertext: row.variables_ciphertext,
            runtime_ref: row.runtime_ref,
            status: row.status,
            failure_reason: row.failure_reason,
            created_at: row.created_at,
            started_at: row.started_at,
            finished_at: row.finished_at,
        }
    }
}

async fn cursor(pool: &SqlitePool, table: &str, deployment_id: &str) -> Result<SequenceCursor> {
    let query = format!("SELECT MIN(sequence), MAX(sequence) FROM {table} WHERE deployment_id = ?");
    let (oldest, newest): (Option<i64>, Option<i64>) = sqlx::query_as(&query)
        .bind(deployment_id)
        .fetch_one(pool)
        .await?;
    Ok(SequenceCursor { oldest, newest })
}

#[derive(Debug, FromRow)]
struct EventRow {
    sequence: i64,
    deployment_id: String,
    kind: String,
    payload_json: String,
    created_at: String,
}

#[derive(Debug, FromRow)]
struct LogRow {
    sequence: i64,
    deployment_id: String,
    stream: String,
    line: String,
    created_at: String,
}
