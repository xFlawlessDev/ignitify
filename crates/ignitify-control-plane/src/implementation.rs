use crate::deployment_retry::schedule_runtime_retry;
use crate::deployment_values::{
    decrypt_deployment_environment, deployment_secret_values, redact_logs,
};
use crate::model::{AgeCipher, Error, Result};
use crate::{DeploymentLogSink, StreamPublisher};
use crate::{
    DnsVerifier, ImageRuntime, Ingress, NoopDnsVerifier, NoopSourceBuild, RuntimeDeployment,
    RuntimeObservation, SourceBuild, reconcile_dns_verifications,
};
#[cfg(test)]
use crate::{ProjectEnvironmentVariableInput, ServiceControl, ServiceMutationOutcomeModel};

use std::{collections::HashSet, time::Duration};

use chrono::{DateTime, Utc};
use ignitify_db::{DeploymentRecord, DeploymentsRepository, DomainsRepository};
use ignitify_domain::{DeploymentState, evaluate_supply_chain_report};

const HEALTH_GATE_TIMEOUT: Duration = Duration::from_secs(300);

pub(crate) struct ReconciliationContext<'a, R, I, S, V> {
    pub(crate) runtime: &'a R,
    pub(crate) ingress: &'a I,
    pub(crate) source_build: &'a S,
    pub(crate) dns_verifier: &'a V,
}

pub async fn reconcile_once<R, I>(
    deployments: &DeploymentsRepository,
    domains: &DomainsRepository,
    cipher: &AgeCipher,
    runtime: &R,
    ingress: &I,
    publisher: &StreamPublisher,
) -> Result<()>
where
    R: ImageRuntime,
    I: Ingress,
{
    reconcile_once_with_source(
        deployments,
        domains,
        cipher,
        runtime,
        ingress,
        &NoopSourceBuild,
        publisher,
    )
    .await
}

pub async fn reconcile_once_with_source<R, I, S>(
    deployments: &DeploymentsRepository,
    domains: &DomainsRepository,
    cipher: &AgeCipher,
    runtime: &R,
    ingress: &I,
    source_build: &S,
    publisher: &StreamPublisher,
) -> Result<()>
where
    R: ImageRuntime,
    I: Ingress,
    S: SourceBuild,
{
    reconcile_once_with_context(
        deployments,
        domains,
        cipher,
        &ReconciliationContext {
            runtime,
            ingress,
            source_build,
            dns_verifier: &NoopDnsVerifier,
        },
        publisher,
    )
    .await
}

async fn reconcile_once_with_context<R, I, S, V>(
    deployments: &DeploymentsRepository,
    domains: &DomainsRepository,
    cipher: &AgeCipher,
    context: &ReconciliationContext<'_, R, I, S, V>,
    publisher: &StreamPublisher,
) -> Result<()>
where
    R: ImageRuntime,
    I: Ingress,
    S: SourceBuild,
    V: DnsVerifier,
{
    reconcile_runtime_state(
        deployments,
        domains,
        cipher,
        context,
        publisher,
        &HashSet::new(),
        true,
    )
    .await?;
    process_next_deployment(
        deployments,
        domains,
        cipher,
        context.runtime,
        context.ingress,
        context.source_build,
        publisher,
    )
    .await
}

pub(crate) async fn reconcile_runtime_state<R, I, S, V>(
    deployments: &DeploymentsRepository,
    domains: &DomainsRepository,
    cipher: &AgeCipher,
    context: &ReconciliationContext<'_, R, I, S, V>,
    publisher: &StreamPublisher,
    active_jobs: &HashSet<String>,
    claim_deployment: bool,
) -> Result<()>
where
    R: ImageRuntime,
    I: Ingress,
    S: SourceBuild,
    V: DnsVerifier,
{
    let runtime = context.runtime;
    let ingress = context.ingress;
    let source_build = context.source_build;
    let dns_verifier = context.dns_verifier;
    ingress.reconcile().await?;
    reconcile_dns_verifications(domains, dns_verifier).await?;
    for deployment in deployments.nonterminal().await? {
        match deployment.state {
            DeploymentState::Queued => {}
            DeploymentState::Preparing | DeploymentState::Running => {
                if let Some(runtime_ref) = deployment.runtime_ref.as_deref() {
                    let runtime_deployment = RuntimeDeployment::from(&deployment);
                    let observation = runtime.inspect(&runtime_deployment, runtime_ref).await?;
                    if observation.owned {
                        persist_logs(
                            deployments,
                            cipher,
                            runtime,
                            &deployment,
                            runtime_ref,
                            publisher,
                        )
                        .await?;
                    }
                    let became_healthy = advance_observed_deployment(
                        deployments,
                        runtime,
                        &deployment,
                        runtime_ref,
                        observation,
                        publisher,
                    )
                    .await?;
                    if became_healthy {
                        cleanup_prior_deployments(deployments, runtime, &deployment, publisher)
                            .await?;
                    }
                } else if deployment.state == DeploymentState::Preparing
                    && !active_jobs.contains(deployment.id.as_str())
                {
                    deployments
                        .reset_preparing_without_runtime(deployment.id.as_str())
                        .await?;
                }
            }
            DeploymentState::Stopping => {
                if let Some(runtime_ref) = deployment.runtime_ref.as_deref() {
                    let stopped = runtime
                        .stop(
                            runtime_ref,
                            deployment.service_id.as_str(),
                            deployment.generation,
                        )
                        .await?;
                    let (next, failure_reason) = if stopped {
                        (DeploymentState::Stopped, None)
                    } else {
                        (DeploymentState::Failed, Some("runtime identity mismatch"))
                    };
                    deployments
                        .transition(
                            deployment.id.as_str(),
                            next,
                            Some(runtime_ref),
                            failure_reason,
                        )
                        .await?;
                    publisher
                        .publish_events(deployments, deployment.id.as_str())
                        .await;
                } else {
                    deployments
                        .transition(deployment.id.as_str(), DeploymentState::Stopped, None, None)
                        .await?;
                    publisher
                        .publish_events(deployments, deployment.id.as_str())
                        .await;
                }
            }
            DeploymentState::Healthy
            | DeploymentState::Failed
            | DeploymentState::Stopped
            | DeploymentState::Superseded => {}
        }
    }
    for deployment in deployments.routable().await? {
        let runtime_deployment = RuntimeDeployment::from(&deployment);
        if deployment.state == DeploymentState::Healthy
            && let Some(runtime_ref) = deployment.runtime_ref.as_deref()
        {
            persist_logs(
                deployments,
                cipher,
                runtime,
                &deployment,
                runtime_ref,
                publisher,
            )
            .await?;
        }
        reconcile_routes(
            domains,
            cipher,
            runtime,
            ingress,
            &deployment,
            &runtime_deployment,
        )
        .await?;
    }
    if !claim_deployment {
        return Ok(());
    }
    let Some(deployment) = deployments.claim_next().await? else {
        return Ok(());
    };
    publisher
        .publish_events(deployments, deployment.id.as_str())
        .await;
    process_claimed_deployment(
        deployments,
        domains,
        cipher,
        runtime,
        ingress,
        source_build,
        publisher,
        deployment,
    )
    .await
}

async fn process_next_deployment<R, I, S>(
    deployments: &DeploymentsRepository,
    domains: &DomainsRepository,
    cipher: &AgeCipher,
    runtime: &R,
    ingress: &I,
    source_build: &S,
    publisher: &StreamPublisher,
) -> Result<()>
where
    R: ImageRuntime,
    I: Ingress,
    S: SourceBuild,
{
    let Some(deployment) = deployments.claim_next().await? else {
        return Ok(());
    };
    publisher
        .publish_events(deployments, deployment.id.as_str())
        .await;
    process_claimed_deployment(
        deployments,
        domains,
        cipher,
        runtime,
        ingress,
        source_build,
        publisher,
        deployment,
    )
    .await
}

#[expect(
    clippy::too_many_arguments,
    reason = "a claimed deployment is processed with explicit adapter boundaries"
)]
pub(crate) async fn process_claimed_deployment<R, I, S>(
    deployments: &DeploymentsRepository,
    domains: &DomainsRepository,
    cipher: &AgeCipher,
    runtime: &R,
    ingress: &I,
    source_build: &S,
    publisher: &StreamPublisher,
    deployment: DeploymentRecord,
) -> Result<()>
where
    R: ImageRuntime,
    I: Ingress,
    S: SourceBuild,
{
    if deployments.cancel_requested(deployment.id.as_str()).await? {
        return Ok(());
    }
    let source_logs = DeploymentLogSink::new(
        deployments.clone(),
        publisher.clone(),
        deployment.id.as_str(),
    );
    source_logs.system("Source build started").await?;
    let (runtime_deployment, source_revision) = match source_build
        .build(&deployment, &source_logs)
        .await
    {
        Ok(Some(output)) => {
            source_logs.system("Source build completed").await?;
            if deployments.cancel_requested(deployment.id.as_str()).await? {
                return Ok(());
            }
            let source_revision = output.source_revision;
            let local_image_id = output.local_image_id;
            let runtime_spec = output.runtime_spec;
            deployments
                .record_source_resolution(
                    deployment.id.as_str(),
                    &source_revision,
                    local_image_id.as_deref(),
                    runtime_spec.as_ref(),
                )
                .await?;
            let mut runtime_deployment = RuntimeDeployment::from(&deployment);
            runtime_deployment.local_image_id = local_image_id;
            if let Some(spec) = runtime_spec {
                runtime_deployment.spec = spec;
            }
            (runtime_deployment, Some(source_revision))
        }
        Ok(None) => (
            RuntimeDeployment::from(&deployment),
            deployment.source_revision.clone(),
        ),
        Err(error) => {
            tracing::warn!(deployment_id = %deployment.id, error = %error, "deployment source build failed");
            if deployments.cancel_requested(deployment.id.as_str()).await? {
                return Ok(());
            }
            let failure_reason = match error {
                Error::SourceBuild(reason) => format!("source build failed: {reason}"),
                Error::Policy(reason) => format!("source build policy rejected: {reason}"),
                error => format!("source build failed: {error}"),
            };
            let _ = source_logs
                .system(format!("Source build failed: {failure_reason}"))
                .await;
            deployments
                .transition(
                    deployment.id.as_str(),
                    DeploymentState::Failed,
                    None,
                    Some(&failure_reason),
                )
                .await?;
            publisher
                .publish_events(deployments, deployment.id.as_str())
                .await;
            return Ok(());
        }
    };
    if deployments.cancel_requested(deployment.id.as_str()).await? {
        return Ok(());
    }
    let policy = deployments.supply_chain_policy().await?;
    let report = evaluate_supply_chain_report(
        &runtime_deployment.spec,
        deployment.source_config.as_ref(),
        source_revision.as_deref(),
        runtime_deployment.local_image_id.as_deref(),
        policy.enforcement,
        Utc::now().to_rfc3339(),
    );
    deployments
        .record_supply_chain_report(deployment.id.as_str(), &report)
        .await?;
    if report.blocks_execution() {
        let failure_reason = "supply-chain policy requires resolved provenance";
        source_logs
            .system("Supply-chain policy blocked runtime execution: provenance is unresolved")
            .await?;
        deployments
            .transition(
                deployment.id.as_str(),
                DeploymentState::Failed,
                None,
                Some(failure_reason),
            )
            .await?;
        publisher
            .publish_events(deployments, deployment.id.as_str())
            .await;
        return Ok(());
    }
    let predicted_runtime_ref = runtime.runtime_ref(&runtime_deployment);
    deployments
        .record_runtime_ref(deployment.id.as_str(), &predicted_runtime_ref)
        .await?;
    let environment = decrypt_deployment_environment(cipher, &deployment.variables_ciphertext)?;
    let runtime_ref = match runtime.start(&runtime_deployment, environment).await {
        Ok(runtime_ref) => {
            deployments
                .replace_runtime_ref(deployment.id.as_str(), &runtime_ref)
                .await?;
            runtime_ref
        }
        Err(Error::Policy(reason)) => {
            let failure_reason = format!("runtime policy rejected input: {reason}");
            deployments
                .transition(
                    deployment.id.as_str(),
                    DeploymentState::Failed,
                    Some(&predicted_runtime_ref),
                    Some(&failure_reason),
                )
                .await?;
            publisher
                .publish_events(deployments, deployment.id.as_str())
                .await;
            return Ok(());
        }
        Err(error) => {
            tracing::warn!(deployment_id = %deployment.id, error = %error, "deployment runtime start uncertain");
            match runtime
                .inspect(&runtime_deployment, &predicted_runtime_ref)
                .await
            {
                Ok(observation) if observation.owned && observation.running => {
                    predicted_runtime_ref
                }
                Ok(_) => {
                    schedule_runtime_retry(deployments, &deployment, publisher).await?;
                    return Ok(());
                }
                Err(inspect_error) => return Err(inspect_error),
            }
        }
    };
    // Cancellation can race with runtime creation. Leave cleanup to the next
    // reconciliation pass after the stopping state and runtime reference are
    // durably recorded.
    if deployments.cancel_requested(deployment.id.as_str()).await? {
        return Ok(());
    }
    let observation = match runtime.inspect(&runtime_deployment, &runtime_ref).await {
        Ok(observation) => observation,
        Err(error) => {
            tracing::warn!(deployment_id = %deployment.id, error = %error, "deployment runtime inspection uncertain");
            return Ok(());
        }
    };
    if observation.owned {
        persist_logs(
            deployments,
            cipher,
            runtime,
            &deployment,
            &runtime_ref,
            publisher,
        )
        .await?;
    }
    let became_healthy = advance_observed_deployment(
        deployments,
        runtime,
        &deployment,
        &runtime_ref,
        observation,
        publisher,
    )
    .await?;
    if observation.owned {
        reconcile_routes(
            domains,
            cipher,
            runtime,
            ingress,
            &deployment,
            &runtime_deployment,
        )
        .await?;
    }
    if became_healthy {
        cleanup_prior_deployments(deployments, runtime, &deployment, publisher).await?;
    }
    Ok(())
}

async fn persist_logs<R>(
    deployments: &DeploymentsRepository,
    cipher: &AgeCipher,
    runtime: &R,
    deployment: &DeploymentRecord,
    runtime_ref: &str,
    publisher: &StreamPublisher,
) -> Result<()>
where
    R: ImageRuntime,
{
    let values = deployment_secret_values(cipher, &deployment.variables_ciphertext)?;
    // ponytail: Docker's `since` cursor is second-granularity, so replay its boundary rather than lose same-second lines. Add source timestamps plus a unique cursor when exact-once logs matter.
    let since = deployments
        .latest_log_since(deployment.id.as_str())
        .await?
        .unwrap_or(0);
    let logs = redact_logs(
        runtime
            .logs(runtime_ref, since)
            .await?
            .into_iter()
            .map(ignitify_db::NewDeploymentLog::from)
            .collect(),
        &values,
    );
    let inserted = deployments
        .append_logs(deployment.id.as_str(), &logs)
        .await?;
    publisher.publish_logs(inserted);
    Ok(())
}

async fn reconcile_routes<R, I>(
    domains: &DomainsRepository,
    cipher: &AgeCipher,
    runtime: &R,
    ingress: &I,
    deployment: &DeploymentRecord,
    runtime_deployment: &RuntimeDeployment,
) -> Result<()>
where
    R: ImageRuntime,
    I: Ingress,
{
    let Some(port) = runtime_deployment.spec.internal_port() else {
        return Ok(());
    };
    let domain_records = domains
        .active_for_service(deployment.service_id.as_str())
        .await?;
    if domain_records.is_empty() {
        return Ok(());
    }
    // A remote release attaches directly to the ingress on its destination. Its
    // runtime verifies that destination network during reconciliation, so local
    // ingress availability must not gate a remote service.
    if deployment.deployment_destination_id.is_none() && !ingress.ensure_ready().await? {
        set_domain_statuses(
            domains,
            &domain_records,
            ignitify_domain::DomainStatus::Failed,
            Some("ingress is unavailable"),
        )
        .await?;
        return Ok(());
    }
    let mut routes = Vec::with_capacity(domain_records.len());
    for domain in &domain_records {
        routes.push(ingress.route(&deployment.service_id, &domain.id, &domain.hostname, port)?);
    }
    let environment = decrypt_deployment_environment(cipher, &deployment.variables_ciphertext)?;
    let Some(runtime_ref) = deployment.runtime_ref.as_deref() else {
        return Ok(());
    };
    match runtime
        .reconcile_routes(runtime_deployment, runtime_ref, environment, routes)
        .await
    {
        Ok(true) => {
            set_domain_statuses(
                domains,
                &domain_records,
                ignitify_domain::DomainStatus::Active,
                None,
            )
            .await
        }
        Ok(false) => {
            set_domain_statuses(
                domains,
                &domain_records,
                ignitify_domain::DomainStatus::Failed,
                Some("route reconciliation failed"),
            )
            .await
        }
        Err(error) => {
            set_domain_statuses(
                domains,
                &domain_records,
                ignitify_domain::DomainStatus::Failed,
                Some("route reconciliation failed"),
            )
            .await?;
            Err(error)
        }
    }
}

async fn set_domain_statuses(
    domains: &DomainsRepository,
    domain_records: &[ignitify_db::DomainRecord],
    status: ignitify_domain::DomainStatus,
    last_error: Option<&str>,
) -> Result<()> {
    for domain in domain_records {
        domains
            .set_status(domain.id.as_str(), status, last_error)
            .await?;
    }
    Ok(())
}

async fn cleanup_prior_deployments<R>(
    deployments: &DeploymentsRepository,
    runtime: &R,
    deployment: &DeploymentRecord,
    publisher: &StreamPublisher,
) -> Result<()>
where
    R: ImageRuntime,
{
    let prior_deployments = deployments
        .healthy_prior_deployments(deployment.service_id.as_str(), deployment.id.as_str())
        .await?;
    for prior in &prior_deployments {
        if let Some(runtime_ref) = prior.runtime_ref.as_deref()
            && !runtime
                .stop(
                    runtime_ref,
                    deployment.service_id.as_str(),
                    prior.generation,
                )
                .await?
        {
            return Ok(());
        }
    }
    deployments
        .supersede_prior_healthy(deployment.service_id.as_str(), deployment.id.as_str())
        .await?;
    publisher
        .publish_events(deployments, deployment.id.as_str())
        .await;
    for prior in prior_deployments {
        publisher
            .publish_events(deployments, prior.id.as_str())
            .await;
    }
    Ok(())
}

async fn advance_observed_deployment<R>(
    deployments: &DeploymentsRepository,
    runtime: &R,
    deployment: &DeploymentRecord,
    runtime_ref: &str,
    observation: RuntimeObservation,
    publisher: &StreamPublisher,
) -> Result<bool>
where
    R: ImageRuntime,
{
    if !observation.owned {
        deployments
            .transition(
                deployment.id.as_str(),
                DeploymentState::Failed,
                Some(runtime_ref),
                Some("runtime identity mismatch"),
            )
            .await?;
        publisher
            .publish_events(deployments, deployment.id.as_str())
            .await;
        return Ok(false);
    }
    if !observation.running {
        if deployment.state == DeploymentState::Preparing {
            if observation.owned {
                runtime
                    .stop(
                        runtime_ref,
                        deployment.service_id.as_str(),
                        deployment.generation,
                    )
                    .await?;
            }
            schedule_runtime_retry(deployments, deployment, publisher).await?;
            return Ok(false);
        }
        if deployments
            .transition(
                deployment.id.as_str(),
                DeploymentState::Failed,
                Some(runtime_ref),
                Some("runtime is not running"),
            )
            .await?
        {
            runtime
                .stop(
                    runtime_ref,
                    deployment.service_id.as_str(),
                    deployment.generation,
                )
                .await?;
        }
        publisher
            .publish_events(deployments, deployment.id.as_str())
            .await;
        return Ok(false);
    }
    if observation.health_failing {
        if deployments
            .transition(
                deployment.id.as_str(),
                DeploymentState::Failed,
                Some(runtime_ref),
                Some("runtime healthcheck failed"),
            )
            .await?
        {
            runtime
                .stop(
                    runtime_ref,
                    deployment.service_id.as_str(),
                    deployment.generation,
                )
                .await?;
        }
        publisher
            .publish_events(deployments, deployment.id.as_str())
            .await;
        return Ok(false);
    }
    if deployment.state == DeploymentState::Preparing {
        deployments
            .transition(
                deployment.id.as_str(),
                DeploymentState::Running,
                Some(runtime_ref),
                None,
            )
            .await?;
    }
    let has_healthcheck = matches!(
        deployment.spec,
        ignitify_domain::ServiceSpec::Image {
            healthcheck: Some(_),
            ..
        }
    );
    if has_healthcheck
        && observation.healthy != Some(true)
        && health_gate_expired(deployment.started_at.as_deref())
    {
        if deployments
            .transition(
                deployment.id.as_str(),
                DeploymentState::Failed,
                Some(runtime_ref),
                Some("runtime healthcheck did not become healthy within 5 minutes"),
            )
            .await?
        {
            runtime
                .stop(
                    runtime_ref,
                    deployment.service_id.as_str(),
                    deployment.generation,
                )
                .await?;
        }
        publisher
            .publish_events(deployments, deployment.id.as_str())
            .await;
        return Ok(false);
    }
    let became_healthy = (!has_healthcheck
        && !matches!(
            deployment.spec,
            ignitify_domain::ServiceSpec::Compose { .. }
        )
        || observation.healthy == Some(true)
        || matches!(
            deployment.spec,
            ignitify_domain::ServiceSpec::Compose { .. }
        ) && observation.healthy.is_none())
        && deployments
            .transition(
                deployment.id.as_str(),
                DeploymentState::Healthy,
                Some(runtime_ref),
                None,
            )
            .await?;
    publisher
        .publish_events(deployments, deployment.id.as_str())
        .await;
    Ok(became_healthy)
}

fn health_gate_expired(started_at: Option<&str>) -> bool {
    started_at
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .is_some_and(|started_at| {
            Utc::now().signed_duration_since(started_at.with_timezone(&Utc))
                >= chrono::Duration::from_std(HEALTH_GATE_TIMEOUT).unwrap_or_default()
        })
}

#[cfg(test)]
#[path = "implementation_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "worker_resilience_tests.rs"]
mod worker_resilience_tests;
