use axum::{Json, extract::State, http::HeaderMap};
use chrono::{DateTime, Utc};
use ignitify_db::{OperationsSummary, RemoteAgentOperationsSummary};
use serde::Serialize;

use crate::{error::ApiError, extract::require_actor, state::AppState};

#[derive(Debug, Serialize)]
pub(crate) struct OperationalHealthResponse {
    generated_at: String,
    control_plane: ComponentHealth,
    runtime: ComponentHealth,
    worker: ComponentHealth,
    ingress: ComponentHealth,
    deployments: DeploymentHealth,
    backup: BackupHealth,
    domains: DomainHealth,
    certificates: CertificateHealth,
    remote_agents: RemoteAgentHealth,
}

#[derive(Debug, Serialize)]
struct ComponentHealth {
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct DeploymentHealth {
    status: &'static str,
    queued_count: i64,
    active_count: i64,
    failed_count: i64,
    failed_retry_count: i64,
    retry_count: i64,
    average_duration_seconds: Option<f64>,
    latest_duration_seconds: Option<f64>,
}

#[derive(Debug, Serialize)]
struct BackupHealth {
    status: &'static str,
    configured: bool,
    enabled: bool,
    schedule_interval_hours: Option<u16>,
    latest_status: Option<String>,
    latest_started_at: Option<String>,
    latest_completed_at: Option<String>,
    latest_age_seconds: Option<i64>,
}

#[derive(Debug, Serialize)]
struct DomainHealth {
    status: &'static str,
    active_count: i64,
    pending_count: i64,
    failed_count: i64,
}

#[derive(Debug, Serialize)]
struct CertificateHealth {
    status: &'static str,
    https_enabled: bool,
    provider: String,
    custom_certificate_selected: bool,
    stored_certificate_count: i64,
}

#[derive(Debug, Serialize)]
struct RemoteAgentHealth {
    status: &'static str,
    server_count: i64,
    online_count: i64,
    offline_count: i64,
    pending_count: i64,
    oldest_heartbeat_at: Option<String>,
    oldest_heartbeat_age_seconds: Option<i64>,
}

pub(crate) async fn health_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<OperationalHealthResponse>, ApiError> {
    require_admin(&state, &headers).await?;

    let operations = state.database.operations();
    let (summary, database, runtime, worker, ingress) = tokio::join!(
        operations.summary(),
        state.database.ping(),
        state.runtime_health.ready(),
        state.worker_health.ready(),
        state.ingress_health.ready(),
    );
    let summary = summary?;
    let generated_at = Utc::now();

    Ok(Json(OperationalHealthResponse {
        generated_at: generated_at.to_rfc3339(),
        control_plane: ComponentHealth {
            status: component_status(database.is_ok()),
        },
        runtime: ComponentHealth {
            status: component_status(runtime),
        },
        worker: ComponentHealth {
            status: component_status(worker),
        },
        ingress: ComponentHealth {
            status: component_status(ingress),
        },
        deployments: deployment_health(&summary, worker),
        backup: backup_health(&summary, generated_at),
        domains: domain_health(&summary),
        certificates: certificate_health(&summary),
        remote_agents: remote_agent_health(&summary.remote_agents, generated_at),
    }))
}

fn deployment_health(summary: &OperationsSummary, worker_ready: bool) -> DeploymentHealth {
    let deployment = &summary.deployments;
    let status = if !worker_ready && deployment.active_count > 0 {
        "stalled"
    } else if deployment.failed_count > 0 {
        "failed"
    } else if deployment.active_count > 0 {
        "active"
    } else {
        "healthy"
    };
    DeploymentHealth {
        status,
        queued_count: deployment.queued_count,
        active_count: deployment.active_count,
        failed_count: deployment.failed_count,
        failed_retry_count: deployment.failed_retry_count,
        retry_count: deployment.retry_count,
        average_duration_seconds: deployment.average_duration_seconds,
        latest_duration_seconds: deployment.latest_duration_seconds,
    }
}

fn backup_health(summary: &OperationsSummary, now: DateTime<Utc>) -> BackupHealth {
    let backup = &summary.backup;
    let latest = backup.latest_scheduled_run.as_ref();
    let latest_timestamp =
        latest.and_then(|run| run.completed_at.as_deref().or(Some(&run.started_at)));
    let latest_age_seconds = latest_timestamp.and_then(|value| age_seconds(value, now));
    let stale_after_seconds = backup
        .schedule_interval_hours
        .map_or(24 * 60 * 60, |hours| i64::from(hours) * 60 * 60 * 2);
    let status = if !backup.configured {
        "not_configured"
    } else if !backup.enabled || backup.schedule_interval_hours.is_none() {
        "disabled"
    } else {
        match latest {
            Some(run) if run.status == "failed" => "failed",
            Some(run) if run.status == "running" => "running",
            Some(_) if latest_age_seconds.is_some_and(|age| age > stale_after_seconds) => "stale",
            Some(_) => "healthy",
            None => "stale",
        }
    };
    BackupHealth {
        status,
        configured: backup.configured,
        enabled: backup.enabled,
        schedule_interval_hours: backup.schedule_interval_hours,
        latest_status: latest.map(|run| run.status.clone()),
        latest_started_at: latest.map(|run| run.started_at.clone()),
        latest_completed_at: latest.and_then(|run| run.completed_at.clone()),
        latest_age_seconds,
    }
}

fn domain_health(summary: &OperationsSummary) -> DomainHealth {
    let domains = &summary.domains;
    DomainHealth {
        status: if domains.failed_count > 0 {
            "failed"
        } else if domains.pending_count > 0 {
            "warning"
        } else {
            "healthy"
        },
        active_count: domains.active_count,
        pending_count: domains.pending_count,
        failed_count: domains.failed_count,
    }
}

fn certificate_health(summary: &OperationsSummary) -> CertificateHealth {
    let certificates = &summary.certificates;
    let status = if !certificates.https_enabled {
        "disabled"
    } else if certificates.provider == "custom" {
        if certificates.custom_certificate_selected && certificates.stored_certificate_count > 0 {
            "healthy"
        } else {
            "warning"
        }
    } else if certificates.provider == "lets-encrypt" {
        "healthy"
    } else {
        "warning"
    };
    CertificateHealth {
        status,
        https_enabled: certificates.https_enabled,
        provider: certificates.provider.clone(),
        custom_certificate_selected: certificates.custom_certificate_selected,
        stored_certificate_count: certificates.stored_certificate_count,
    }
}

fn remote_agent_health(
    agents: &RemoteAgentOperationsSummary,
    now: DateTime<Utc>,
) -> RemoteAgentHealth {
    RemoteAgentHealth {
        status: if agents.server_count == 0 {
            "not_configured"
        } else if agents.offline_count > 0 {
            "failed"
        } else if agents.pending_count > 0 {
            "warning"
        } else {
            "healthy"
        },
        server_count: agents.server_count,
        online_count: agents.online_count,
        offline_count: agents.offline_count,
        pending_count: agents.pending_count,
        oldest_heartbeat_at: agents.oldest_heartbeat_at.clone(),
        oldest_heartbeat_age_seconds: agents
            .oldest_heartbeat_at
            .as_deref()
            .and_then(|value| age_seconds(value, now)),
    }
}

fn component_status(ready: bool) -> &'static str {
    if ready { "ready" } else { "unavailable" }
}

fn age_seconds(value: &str, now: DateTime<Utc>) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| (now - timestamp.with_timezone(&Utc)).num_seconds().max(0))
}

async fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    if require_actor(state, headers)
        .await?
        .has_platform_operator_access()
    {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use ignitify_db::{
        BackupOperationsSummary, BackupRunSummary, CertificateOperationsSummary,
        DeploymentOperationsSummary, DomainOperationsSummary, RemoteAgentOperationsSummary,
    };

    use super::*;

    fn summary() -> OperationsSummary {
        OperationsSummary {
            deployments: DeploymentOperationsSummary {
                queued_count: 0,
                active_count: 0,
                failed_count: 0,
                failed_retry_count: 0,
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
    fn deployment_health_marks_an_unready_worker_with_active_work_as_stalled() {
        let mut summary = summary();
        summary.deployments.active_count = 1;
        summary.deployments.failed_count = 1;

        assert_eq!(deployment_health(&summary, false).status, "stalled");
    }

    #[test]
    fn backup_health_marks_an_overdue_scheduled_run_as_stale() {
        let now = Utc::now();
        let mut summary = summary();
        summary.backup = BackupOperationsSummary {
            configured: true,
            enabled: true,
            schedule_interval_hours: Some(1),
            latest_scheduled_run: Some(BackupRunSummary {
                status: "succeeded".to_owned(),
                started_at: (now - Duration::hours(3)).to_rfc3339(),
                completed_at: Some((now - Duration::hours(3)).to_rfc3339()),
            }),
        };

        assert_eq!(backup_health(&summary, now).status, "stale");
    }

    #[test]
    fn remote_agent_health_marks_offline_agents_as_failed() {
        let now = Utc::now();
        let agents = RemoteAgentOperationsSummary {
            server_count: 1,
            online_count: 0,
            offline_count: 1,
            pending_count: 0,
            oldest_heartbeat_at: Some((now - Duration::minutes(5)).to_rfc3339()),
        };

        assert_eq!(remote_agent_health(&agents, now).status, "failed");
    }
}
