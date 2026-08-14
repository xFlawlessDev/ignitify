use std::collections::BTreeMap;

use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use super::{
    AuthorizedDeploymentService, CreateDeploymentOutcome, DeploymentActor, DeploymentRecord,
    DeploymentRow, DeploymentWithProjectRow, DeploymentsRepository, NewDeployment,
    ProjectVariableRow, ServiceRow, VariableRow, decode_source_config, decode_spec,
    deployment_from_row, fetch_by_service_key, fetch_deployment, history_limit, insert_audit,
    insert_event, next_generation, parse_service_id,
};
use crate::Result;

impl DeploymentsRepository {
    pub async fn service_for_deployment(
        &self,
        actor: DeploymentActor<'_>,
        service_id: &str,
    ) -> Result<Option<AuthorizedDeploymentService>> {
        let row = sqlx::query_as::<_, ServiceRow>(
            "SELECT s.id, e.project_id, s.desired_generation, s.desired_spec_json, s.source_config_json,
                    s.deployment_destination_id
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
        let source_config = decode_source_config(row.source_config_json)?;
        let project_variables = sqlx::query_as::<_, ProjectVariableRow>(
            "SELECT key, ciphertext
             FROM project_variables
             WHERE project_id = ?
             ORDER BY key",
        )
        .bind(&row.project_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(ProjectVariableRow::into_record);
        let service_variables = sqlx::query_as::<_, VariableRow>(
            "SELECT key, ciphertext FROM service_variables WHERE service_id = ? ORDER BY key",
        )
        .bind(&row.id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(VariableRow::into_record);
        let variables = project_variables
            .chain(service_variables)
            .map(|variable| (variable.key.clone(), variable))
            .collect::<BTreeMap<_, _>>()
            .into_values()
            .collect();
        Ok(Some(AuthorizedDeploymentService {
            id: parse_service_id(row.id)?,
            role,
            desired_generation: row.desired_generation,
            spec,
            source_config,
            deployment_destination_id: row.deployment_destination_id,
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
        let correlation_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let spec_json = serde_json::to_string(&deployment.spec)
            .map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
        let source_config_json = deployment
            .source_config
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
        let source_revision = deployment.source_revision.as_deref();
        let inserted = sqlx::query(
            "INSERT INTO deployments (
                id, correlation_id, service_id, generation, idempotency_key, requested_by_user_id, spec_json,
                source_config_json, deployment_destination_id, source_revision, variables_ciphertext, status, created_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'queued', ?)
             ON CONFLICT(service_id, idempotency_key) DO NOTHING
             ON CONFLICT(service_id) WHERE status IN ('queued', 'preparing', 'running') DO NOTHING",
        )
        .bind(&id)
        .bind(&correlation_id)
        .bind(service.id.as_str())
        .bind(generation)
        .bind(&deployment.idempotency_key)
        .bind(&deployment.requested_by_user_id)
        .bind(spec_json)
        .bind(source_config_json)
        .bind(&deployment.deployment_destination_id)
        .bind(source_revision)
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
            &correlation_id,
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
            "SELECT d.id, d.correlation_id, d.service_id, d.generation, d.idempotency_key, d.requested_by_user_id,
                    d.spec_json, d.runtime_spec_json, d.source_config_json, d.deployment_destination_id, d.source_revision, d.local_image_id,
                    d.variables_ciphertext, d.runtime_ref, d.status, d.failure_reason,
                    d.attempt_count, d.retry_after, d.cancel_requested_at,
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
            "SELECT id, correlation_id, service_id, generation, idempotency_key, requested_by_user_id, spec_json, runtime_spec_json,
                    source_config_json, deployment_destination_id, source_revision, local_image_id, variables_ciphertext, runtime_ref,
                    status, failure_reason, attempt_count, retry_after, cancel_requested_at,
                    created_at, started_at, finished_at
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
            "SELECT d.id, d.correlation_id, d.service_id, d.generation, d.idempotency_key, d.requested_by_user_id,
                    d.spec_json, d.runtime_spec_json, d.source_config_json, d.deployment_destination_id, d.source_revision, d.local_image_id,
                    d.variables_ciphertext, d.runtime_ref, d.status, d.failure_reason,
                    d.attempt_count, d.retry_after, d.cancel_requested_at,
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
            "SELECT id, correlation_id, service_id, generation, idempotency_key, requested_by_user_id, spec_json, runtime_spec_json,
                    source_config_json, deployment_destination_id, source_revision, local_image_id, variables_ciphertext, runtime_ref,
                    status, failure_reason, attempt_count, retry_after, cancel_requested_at,
                    created_at, started_at, finished_at
             FROM deployments
             WHERE service_id = ? AND status IN ('queued', 'preparing', 'running', 'healthy', 'stopping')
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(service_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(deployment_from_row).transpose()
    }
}
