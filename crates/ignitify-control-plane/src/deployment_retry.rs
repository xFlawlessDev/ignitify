use ignitify_db::{DeploymentRecord, DeploymentsRepository, RetrySchedule};

use crate::{Result, StreamPublisher};

const MAX_RUNTIME_START_ATTEMPTS: i64 = 3;

pub(crate) async fn schedule_runtime_retry(
    deployments: &DeploymentsRepository,
    deployment: &DeploymentRecord,
    publisher: &StreamPublisher,
) -> Result<()> {
    match deployments
        .schedule_retry(
            deployment.id.as_str(),
            "runtime did not start",
            MAX_RUNTIME_START_ATTEMPTS,
        )
        .await?
    {
        RetrySchedule::Scheduled { retry_after } => {
            tracing::warn!(
                deployment_id = %deployment.id,
                attempt_count = deployment.attempt_count,
                retry_after = %retry_after,
                "deployment runtime start retry scheduled"
            );
        }
        RetrySchedule::Exhausted => {
            tracing::warn!(deployment_id = %deployment.id, "deployment runtime retries exhausted");
        }
        RetrySchedule::Cancelled | RetrySchedule::Unchanged => {}
    }
    publisher
        .publish_events(deployments, deployment.id.as_str())
        .await;
    Ok(())
}
