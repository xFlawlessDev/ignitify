use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use ignitify_control_plane::RuntimeHealth;
use ignitify_db::{BackupOperationsSummary, Database, OperationsSummary};

use crate::{AgeCipher, NotificationEvent, Result, dispatch};

const POLL_INTERVAL: Duration = Duration::from_secs(30);
const RETRY_EXHAUSTION_WINDOW_SECONDS: i64 = 30 * 60;

#[derive(Debug, Clone, Copy)]
enum OperationalAlert {
    WorkerStalled,
    RetryExhausted,
    BackupStale,
    RemoteAgentOffline,
    DomainVerificationFailed,
    CertificateNeedsAttention,
}

const ALERTS: &[OperationalAlert] = &[
    OperationalAlert::WorkerStalled,
    OperationalAlert::RetryExhausted,
    OperationalAlert::BackupStale,
    OperationalAlert::RemoteAgentOffline,
    OperationalAlert::DomainVerificationFailed,
    OperationalAlert::CertificateNeedsAttention,
];

impl OperationalAlert {
    fn key(self) -> &'static str {
        match self {
            Self::WorkerStalled => "deployment.worker_stalled",
            Self::RetryExhausted => "deployment.retry_exhausted",
            Self::BackupStale => "backup.stale",
            Self::RemoteAgentOffline => "remote_agent.offline",
            Self::DomainVerificationFailed => "domain.verification_failed",
            Self::CertificateNeedsAttention => "certificate.needs_attention",
        }
    }

    fn from_key(value: &str) -> Option<Self> {
        ALERTS.iter().copied().find(|alert| alert.key() == value)
    }

    fn active(self, summary: &OperationsSummary, worker_ready: bool, now: DateTime<Utc>) -> bool {
        match self {
            Self::WorkerStalled => !worker_ready && summary.deployments.active_count > 0,
            Self::RetryExhausted => summary.deployments.recent_failed_retry_count > 0,
            Self::BackupStale => scheduled_backup_is_stale(&summary.backup, now),
            Self::RemoteAgentOffline => summary.remote_agents.offline_count > 0,
            Self::DomainVerificationFailed => summary.domains.failed_count > 0,
            Self::CertificateNeedsAttention => {
                let certificate = &summary.certificates;
                certificate.https_enabled
                    && (certificate.provider == "custom"
                        && (!certificate.custom_certificate_selected
                            || certificate.stored_certificate_count == 0))
            }
        }
    }

    fn raised_title(self) -> &'static str {
        match self {
            Self::WorkerStalled => "Deployment worker stalled",
            Self::RetryExhausted => "Deployment retries exhausted",
            Self::BackupStale => "Scheduled backup needs attention",
            Self::RemoteAgentOffline => "Remote agent offline",
            Self::DomainVerificationFailed => "Domain verification failed",
            Self::CertificateNeedsAttention => "HTTPS certificate needs attention",
        }
    }

    fn raised_body(self, summary: &OperationsSummary) -> String {
        match self {
            Self::WorkerStalled => format!(
                "Ignitify has {} active deployment(s), but the deployment worker is unavailable.",
                summary.deployments.active_count
            ),
            Self::RetryExhausted => format!(
                "Ignitify observed {} deployment retry exhaustion(s) in the last {} minutes.",
                summary.deployments.recent_failed_retry_count,
                RETRY_EXHAUSTION_WINDOW_SECONDS / 60
            ),
            Self::BackupStale => {
                "The latest scheduled backup is unavailable, failed, or older than two scheduled intervals."
                    .to_owned()
            }
            Self::RemoteAgentOffline => format!(
                "Ignitify reports {} remote agent(s) offline.",
                summary.remote_agents.offline_count
            ),
            Self::DomainVerificationFailed => format!(
                "Ignitify reports {} domain(s) with failed verification.",
                summary.domains.failed_count
            ),
            Self::CertificateNeedsAttention => {
                "HTTPS is enabled, but the configured custom certificate is missing or incomplete."
                    .to_owned()
            }
        }
    }
}

pub fn spawn_operational_alert_dispatcher(
    database: Database,
    cipher: Arc<AgeCipher>,
    worker_health: Arc<dyn RuntimeHealth>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(POLL_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            if let Err(error) =
                evaluate_and_dispatch(&database, cipher.as_ref(), &*worker_health).await
            {
                tracing::warn!(error = %error, "operational alert dispatcher cycle failed");
            }
        }
    })
}

async fn evaluate_and_dispatch(
    database: &Database,
    cipher: &AgeCipher,
    worker_health: &dyn RuntimeHealth,
) -> Result<()> {
    let operations = database.operations();
    let (summary, worker_ready) = tokio::join!(operations.summary(), worker_health.ready());
    let summary = summary?;
    let now = Utc::now();

    for alert in ALERTS {
        let _ = operations
            .transition_alert(alert.key(), alert.active(&summary, worker_ready, now))
            .await?;
    }

    for event in operations.pending_alert_events(100).await? {
        let Some(alert) = OperationalAlert::from_key(&event.alert_key) else {
            tracing::warn!(alert_key = %event.alert_key, "unknown operational alert event discarded");
            operations
                .finish_alert_event(&event.alert_key, event.generation, &event.kind)
                .await?;
            continue;
        };
        let (title, body) = match event.kind.as_str() {
            "raised" => (alert.raised_title(), alert.raised_body(&summary)),
            "resolved" => (
                "Operational alert resolved",
                format!(
                    "Ignitify operational alert resolved: {}.",
                    alert.raised_title()
                ),
            ),
            _ => {
                tracing::warn!(kind = %event.kind, alert_key = %event.alert_key, "invalid operational alert event discarded");
                operations
                    .finish_alert_event(&event.alert_key, event.generation, &event.kind)
                    .await?;
                continue;
            }
        };
        let source_id = format!(
            "operations/{}/{}/{}",
            event.alert_key, event.generation, event.kind
        );
        dispatch(
            database,
            cipher,
            NotificationEvent {
                source_kind: "operations",
                source_id: &source_id,
                correlation_id: Some(&source_id),
                event_kind: "operations.alert",
                occurred_at: None,
                title,
                body,
            },
        )
        .await?;
        operations
            .finish_alert_event(&event.alert_key, event.generation, &event.kind)
            .await?;
    }
    Ok(())
}

fn scheduled_backup_is_stale(backup: &BackupOperationsSummary, now: DateTime<Utc>) -> bool {
    if !backup.configured || !backup.enabled || backup.schedule_interval_hours.is_none() {
        return false;
    }
    let Some(run) = &backup.latest_scheduled_run else {
        return true;
    };
    if run.status == "failed" {
        return true;
    }
    let Some(reference) = run
        .completed_at
        .as_deref()
        .or(Some(run.started_at.as_str()))
    else {
        return true;
    };
    let Ok(reference) = DateTime::parse_from_rfc3339(reference) else {
        return true;
    };
    let stale_after = i64::from(backup.schedule_interval_hours.unwrap_or_default()) * 2 * 60 * 60;
    now.signed_duration_since(reference.with_timezone(&Utc))
        .num_seconds()
        > stale_after
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use ignitify_db::{
        BackupOperationsSummary, BackupRunSummary, CertificateOperationsSummary,
        DeploymentOperationsSummary, DomainOperationsSummary, OperationsSummary,
        RemoteAgentOperationsSummary,
    };

    use super::*;

    fn summary() -> OperationsSummary {
        OperationsSummary {
            deployments: DeploymentOperationsSummary {
                queued_count: 0,
                active_count: 0,
                failed_count: 0,
                failed_retry_count: 0,
                recent_failed_retry_count: 0,
                retry_count: 0,
                average_duration_seconds: None,
                latest_duration_seconds: None,
            },
            backup: BackupOperationsSummary {
                configured: false,
                enabled: false,
                schedule_interval_hours: None,
                latest_scheduled_run: None,
            },
            domains: DomainOperationsSummary {
                active_count: 0,
                pending_count: 0,
                failed_count: 0,
            },
            certificates: CertificateOperationsSummary {
                https_enabled: false,
                provider: "none".to_owned(),
                custom_certificate_selected: false,
                stored_certificate_count: 0,
            },
            remote_agents: RemoteAgentOperationsSummary {
                server_count: 0,
                online_count: 0,
                offline_count: 0,
                pending_count: 0,
                oldest_heartbeat_at: None,
            },
        }
    }

    #[test]
    fn alert_thresholds_require_an_active_operational_condition() {
        let now = Utc::now();
        let mut value = summary();
        value.deployments.active_count = 1;
        assert!(OperationalAlert::WorkerStalled.active(&value, false, now));
        assert!(!OperationalAlert::WorkerStalled.active(&value, true, now));

        value.deployments.recent_failed_retry_count = 1;
        assert!(OperationalAlert::RetryExhausted.active(&value, true, now));

        value.backup = BackupOperationsSummary {
            configured: true,
            enabled: true,
            schedule_interval_hours: Some(1),
            latest_scheduled_run: Some(BackupRunSummary {
                status: "succeeded".to_owned(),
                started_at: (now - Duration::hours(3)).to_rfc3339(),
                completed_at: Some((now - Duration::hours(3)).to_rfc3339()),
            }),
        };
        assert!(OperationalAlert::BackupStale.active(&value, true, now));

        value.remote_agents.offline_count = 1;
        assert!(OperationalAlert::RemoteAgentOffline.active(&value, true, now));
        value.domains.failed_count = 1;
        assert!(OperationalAlert::DomainVerificationFailed.active(&value, true, now));
    }
}
