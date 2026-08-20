use chrono::Utc;
use ignitify_domain::{ServiceSpec, SupplyChainReport};
use serde_json::json;

use super::{
    CancelDeploymentOutcome, CreateDeploymentOutcome, DatabaseError, DeploymentActor,
    DeploymentApprovalOutcome, DeploymentRecord, DeploymentRow, DeploymentState,
    DeploymentsRepository, NewDeployment, ProjectMemberRole, Result, RollbackArtifact,
    deployment_from_row, fetch_deployment, insert_audit, insert_event,
};

impl DeploymentsRepository {
    pub async fn claim_next(&self) -> Result<Option<DeploymentRecord>> {
        let mut tx = self.pool.begin().await?;
        let now = Utc::now().to_rfc3339();
        let id: Option<String> = sqlx::query_scalar(
            "SELECT id FROM deployments
             WHERE status = 'queued' AND approval_status != 'pending'
               AND (retry_after IS NULL OR retry_after <= ?)
             ORDER BY created_at LIMIT 1",
        )
        .bind(&now)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(id) = id else {
            tx.commit().await?;
            return Ok(None);
        };
        let changed = sqlx::query(
            "UPDATE deployments
             SET status = 'preparing', started_at = ?, retry_after = NULL,
                 attempt_count = attempt_count + 1
             WHERE id = ? AND status = 'queued' AND approval_status != 'pending'
               AND (retry_after IS NULL OR retry_after <= ?)",
        )
        .bind(&now)
        .bind(&id)
        .bind(&now)
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

    pub async fn cancel(
        &self,
        actor: DeploymentActor<'_>,
        deployment_id: &str,
    ) -> Result<CancelDeploymentOutcome> {
        let Some(current) = self.get(actor, deployment_id).await? else {
            return Ok(CancelDeploymentOutcome::Missing);
        };
        let Some(service) = self
            .service_for_deployment(actor, current.service_id.as_str())
            .await?
        else {
            return Ok(CancelDeploymentOutcome::Missing);
        };
        if !actor.is_admin && !service.role.can_manage_services() {
            return Ok(CancelDeploymentOutcome::Forbidden);
        }

        let mut tx = self.pool.begin().await?;
        let Some(current) = fetch_deployment(&mut tx, deployment_id).await? else {
            tx.commit().await?;
            return Ok(CancelDeploymentOutcome::Missing);
        };
        if matches!(
            current.state,
            DeploymentState::Failed
                | DeploymentState::Stopped
                | DeploymentState::Superseded
                | DeploymentState::Stopping
        ) {
            tx.commit().await?;
            return Ok(CancelDeploymentOutcome::Existing(current));
        }

        let now = Utc::now().to_rfc3339();
        let next = if current.state == DeploymentState::Queued {
            DeploymentState::Stopped
        } else {
            DeploymentState::Stopping
        };
        sqlx::query(
            "UPDATE deployments
             SET status = ?, cancel_requested_at = ?, retry_after = NULL,
                 finished_at = CASE WHEN ? THEN ? ELSE finished_at END
             WHERE id = ? AND status = ?",
        )
        .bind(next.as_str())
        .bind(&now)
        .bind(next.is_terminal())
        .bind(&now)
        .bind(deployment_id)
        .bind(current.state.as_str())
        .execute(&mut *tx)
        .await?;
        let event_kind = if next == DeploymentState::Stopped {
            "deployment.cancelled"
        } else {
            "deployment.cancellation_requested"
        };
        insert_event(&mut tx, deployment_id, event_kind, json!({}), &now).await?;
        insert_audit(
            &mut tx,
            actor.id,
            "deployment.cancel",
            deployment_id,
            &current.correlation_id,
            &now,
        )
        .await?;
        let record = fetch_deployment(&mut tx, deployment_id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
        tx.commit().await?;
        Ok(CancelDeploymentOutcome::Cancelled(record))
    }

    pub async fn approve(
        &self,
        actor: DeploymentActor<'_>,
        deployment_id: &str,
    ) -> Result<DeploymentApprovalOutcome> {
        let Some(current) = self.get(actor, deployment_id).await? else {
            return Ok(DeploymentApprovalOutcome::Missing);
        };
        let Some(service) = self
            .service_for_deployment(actor, current.service_id.as_str())
            .await?
        else {
            return Ok(DeploymentApprovalOutcome::Missing);
        };
        if !actor.is_admin && service.role != ProjectMemberRole::Owner {
            return Ok(DeploymentApprovalOutcome::Forbidden);
        }

        let mut tx = self.pool.begin().await?;
        let Some(current) = fetch_deployment(&mut tx, deployment_id).await? else {
            tx.commit().await?;
            return Ok(DeploymentApprovalOutcome::Missing);
        };
        if current.state != DeploymentState::Queued || !current.approval.is_pending() {
            tx.commit().await?;
            return Ok(DeploymentApprovalOutcome::Existing(current));
        }

        let now = Utc::now().to_rfc3339();
        let changed = sqlx::query(
            "UPDATE deployments
             SET approval_status = 'approved', approved_by_user_id = ?, approved_at = ?
             WHERE id = ? AND status = 'queued' AND approval_status = 'pending'",
        )
        .bind(actor.id)
        .bind(&now)
        .bind(deployment_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if changed == 0 {
            let record = fetch_deployment(&mut tx, deployment_id)
                .await?
                .ok_or(sqlx::Error::RowNotFound)?;
            tx.commit().await?;
            return Ok(DeploymentApprovalOutcome::Existing(record));
        }
        insert_event(
            &mut tx,
            deployment_id,
            "deployment.approved",
            json!({}),
            &now,
        )
        .await?;
        insert_event(&mut tx, deployment_id, "deployment.queued", json!({}), &now).await?;
        insert_audit(
            &mut tx,
            actor.id,
            "deployment.approve",
            deployment_id,
            &current.correlation_id,
            &now,
        )
        .await?;
        let record = fetch_deployment(&mut tx, deployment_id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
        tx.commit().await?;
        Ok(DeploymentApprovalOutcome::Approved(record))
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

    pub async fn record_source_build(
        &self,
        deployment_id: &str,
        source_revision: &str,
        local_image_id: &str,
    ) -> Result<bool> {
        let changed = sqlx::query(
            "UPDATE deployments
             SET source_revision = COALESCE(source_revision, ?), local_image_id = ?
             WHERE id = ? AND status = 'preparing' AND runtime_ref IS NULL",
        )
        .bind(source_revision)
        .bind(local_image_id)
        .bind(deployment_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(changed == 1)
    }

    pub async fn record_source_resolution(
        &self,
        deployment_id: &str,
        source_revision: &str,
        local_image_id: Option<&str>,
        runtime_spec: Option<&ServiceSpec>,
    ) -> Result<bool> {
        let runtime_spec_json = runtime_spec
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
        let changed = sqlx::query(
            "UPDATE deployments
             SET source_revision = COALESCE(source_revision, ?),
                 local_image_id = COALESCE(local_image_id, ?),
                 runtime_spec_json = COALESCE(?, runtime_spec_json)
             WHERE id = ? AND status = 'preparing' AND runtime_ref IS NULL",
        )
        .bind(source_revision)
        .bind(local_image_id)
        .bind(runtime_spec_json)
        .bind(deployment_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(changed == 1)
    }

    pub async fn record_supply_chain_report(
        &self,
        deployment_id: &str,
        report: &SupplyChainReport,
    ) -> Result<bool> {
        let report_json = serde_json::to_string(report)
            .map_err(|error| sqlx::Error::Protocol(error.to_string()))?;
        let changed = sqlx::query(
            "UPDATE deployments
             SET supply_chain_report_json = ?
             WHERE id = ? AND status = 'preparing'",
        )
        .bind(report_json)
        .bind(deployment_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(changed == 1)
    }

    pub async fn record_source_revision(
        &self,
        deployment_id: &str,
        source_revision: &str,
    ) -> Result<bool> {
        let changed = sqlx::query(
            "UPDATE deployments
             SET source_revision = COALESCE(source_revision, ?)
             WHERE id = ? AND status = 'preparing' AND runtime_ref IS NULL",
        )
        .bind(source_revision)
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
            "SELECT id, correlation_id, service_id, generation, idempotency_key, rollback_of_deployment_id, requested_by_user_id, spec_json, runtime_spec_json,
                    source_config_json, deployment_destination_id, source_revision, local_image_id, supply_chain_report_json,
                    approval_status, approval_requested_at, approved_by_user_id, approved_at, variables_ciphertext, runtime_ref,
                    status, failure_reason, attempt_count, retry_after, cancel_requested_at,
                    created_at, started_at, finished_at
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
            "SELECT id, correlation_id, service_id, generation, idempotency_key, rollback_of_deployment_id, requested_by_user_id, spec_json, runtime_spec_json,
                    source_config_json, deployment_destination_id, source_revision, local_image_id, supply_chain_report_json,
                    approval_status, approval_requested_at, approved_by_user_id, approved_at, variables_ciphertext, runtime_ref,
                    status, failure_reason, attempt_count, retry_after, cancel_requested_at,
                    created_at, started_at, finished_at
             FROM deployments WHERE status IN ('running', 'healthy') AND runtime_ref IS NOT NULL",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(deployment_from_row).collect()
    }

    pub async fn nonterminal(&self) -> Result<Vec<DeploymentRecord>> {
        let rows = sqlx::query_as::<_, DeploymentRow>(
            "SELECT id, correlation_id, service_id, generation, idempotency_key, rollback_of_deployment_id, requested_by_user_id, spec_json, runtime_spec_json,
                    source_config_json, deployment_destination_id, source_revision, local_image_id, supply_chain_report_json,
                    approval_status, approval_requested_at, approved_by_user_id, approved_at, variables_ciphertext, runtime_ref,
                    status, failure_reason, attempt_count, retry_after, cancel_requested_at,
                    created_at, started_at, finished_at
             FROM deployments
             WHERE status IN ('queued', 'preparing', 'running', 'stopping')
             ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(deployment_from_row).collect()
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
        let artifact_available =
            source
                .source_config
                .as_ref()
                .is_some_and(|config| match config.source.as_str() {
                    "application" => {
                        source.local_image_id.is_some() && source.source_revision.is_some()
                    }
                    "compose" => config.provider_id.is_some() && source.source_revision.is_some(),
                    _ => false,
                });
        let rollback_artifact = RollbackArtifact {
            source_deployment_id: source.id.to_string(),
            local_image_id: artifact_available
                .then(|| source.local_image_id.clone())
                .flatten(),
            runtime_spec: artifact_available.then(|| source.spec.clone()),
        };
        self.create_with_rollback(
            actor,
            source.service_id.as_str(),
            NewDeployment {
                idempotency_key: idempotency_key.to_owned(),
                requested_by_user_id: actor.id.to_owned(),
                spec: source.spec,
                source_config: source.source_config,
                deployment_destination_id: source.deployment_destination_id,
                source_revision: source.source_revision,
                supply_chain_report: source.supply_chain_report,
                variables_ciphertext: source.variables_ciphertext,
            },
            Some(rollback_artifact),
        )
        .await
    }
}
