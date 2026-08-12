use chrono::Utc;
use serde_json::json;

use super::{
    DeploymentState, DeploymentsRepository, Result, RetrySchedule, fetch_deployment, insert_event,
};

impl DeploymentsRepository {
    pub async fn schedule_retry(
        &self,
        deployment_id: &str,
        reason: &str,
        max_attempts: i64,
    ) -> Result<RetrySchedule> {
        let mut tx = self.pool.begin().await?;
        let Some(current) = fetch_deployment(&mut tx, deployment_id).await? else {
            tx.commit().await?;
            return Ok(RetrySchedule::Unchanged);
        };
        if current.state != DeploymentState::Preparing {
            tx.commit().await?;
            return Ok(RetrySchedule::Unchanged);
        }
        let now = Utc::now().to_rfc3339();
        if current.cancel_requested_at.is_some() {
            sqlx::query(
                "UPDATE deployments
                 SET status = 'stopped', finished_at = ?, retry_after = NULL
                 WHERE id = ? AND status = 'preparing'",
            )
            .bind(&now)
            .bind(deployment_id)
            .execute(&mut *tx)
            .await?;
            insert_event(
                &mut tx,
                deployment_id,
                "deployment.cancelled",
                json!({}),
                &now,
            )
            .await?;
            tx.commit().await?;
            return Ok(RetrySchedule::Cancelled);
        }
        if current.attempt_count >= max_attempts {
            sqlx::query(
                "UPDATE deployments
                 SET status = 'failed', failure_reason = ?, finished_at = ?, retry_after = NULL
                 WHERE id = ? AND status = 'preparing'",
            )
            .bind(format!("{reason} after {max_attempts} attempts"))
            .bind(&now)
            .bind(deployment_id)
            .execute(&mut *tx)
            .await?;
            insert_event(
                &mut tx,
                deployment_id,
                "deployment.failed",
                json!({ "failure_reason": reason, "attempt_count": current.attempt_count }),
                &now,
            )
            .await?;
            tx.commit().await?;
            return Ok(RetrySchedule::Exhausted);
        }

        let delay_seconds = 5_i64 * (1_i64 << current.attempt_count.saturating_sub(1).min(2));
        let retry_after = (Utc::now() + chrono::Duration::seconds(delay_seconds)).to_rfc3339();
        sqlx::query(
            "UPDATE deployments
             SET status = 'queued', runtime_ref = NULL, started_at = NULL,
                 failure_reason = NULL, retry_after = ?
             WHERE id = ? AND status = 'preparing'",
        )
        .bind(&retry_after)
        .bind(deployment_id)
        .execute(&mut *tx)
        .await?;
        insert_event(
            &mut tx,
            deployment_id,
            "deployment.retry_scheduled",
            json!({
                "reason": reason,
                "attempt_count": current.attempt_count,
                "retry_after": retry_after,
            }),
            &now,
        )
        .await?;
        tx.commit().await?;
        Ok(RetrySchedule::Scheduled { retry_after })
    }

    pub async fn cancel_requested(&self, deployment_id: &str) -> Result<bool> {
        Ok(sqlx::query_scalar(
            "SELECT cancel_requested_at IS NOT NULL FROM deployments WHERE id = ?",
        )
        .bind(deployment_id)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(false))
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
}
