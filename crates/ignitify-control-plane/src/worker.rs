use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use ignitify_db::{DeploymentsRepository, DomainsRepository};
use tokio::{sync::mpsc, task::JoinSet};

use crate::implementation::{
    ReconciliationContext, process_claimed_deployment, reconcile_runtime_state,
};
use crate::{
    AgeCipher, DnsVerifier, ImageRuntime, Ingress, NoopSourceBuild, SourceBuild, StreamPublisher,
    WorkerDependencies,
};

const MAX_CONCURRENT_DEPLOYMENT_JOBS: usize = 32;

pub fn spawn_worker<R, I>(
    deployments: DeploymentsRepository,
    domains: DomainsRepository,
    cipher: Arc<AgeCipher>,
    runtime: R,
    ingress: I,
    publisher: StreamPublisher,
    wake: mpsc::Receiver<()>,
) -> (tokio::task::JoinHandle<()>, Arc<AtomicBool>)
where
    R: ImageRuntime,
    I: Ingress,
{
    spawn_worker_with_source(
        deployments,
        domains,
        cipher,
        WorkerDependencies::new(runtime, ingress, NoopSourceBuild),
        publisher,
        wake,
    )
}

pub fn spawn_worker_with_source<R, I, S>(
    deployments: DeploymentsRepository,
    domains: DomainsRepository,
    cipher: Arc<AgeCipher>,
    dependencies: WorkerDependencies<R, I, S>,
    publisher: StreamPublisher,
    wake: mpsc::Receiver<()>,
) -> (tokio::task::JoinHandle<()>, Arc<AtomicBool>)
where
    R: ImageRuntime,
    I: Ingress,
    S: SourceBuild,
{
    spawn_worker_with_source_and_dns(deployments, domains, cipher, dependencies, publisher, wake)
}

pub fn spawn_worker_with_source_and_dns<R, I, S, V>(
    deployments: DeploymentsRepository,
    domains: DomainsRepository,
    cipher: Arc<AgeCipher>,
    dependencies: WorkerDependencies<R, I, S, V>,
    publisher: StreamPublisher,
    mut wake: mpsc::Receiver<()>,
) -> (tokio::task::JoinHandle<()>, Arc<AtomicBool>)
where
    R: ImageRuntime,
    I: Ingress,
    S: SourceBuild,
    V: DnsVerifier,
{
    let ready = Arc::new(AtomicBool::new(false));
    let worker_ready = ready.clone();
    let handle = tokio::spawn(async move {
        struct Liveness(Arc<AtomicBool>);

        impl Drop for Liveness {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }

        let _liveness = Liveness(worker_ready.clone());
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        let mut jobs = JoinSet::new();
        let mut active_jobs = HashSet::new();
        loop {
            let reconciliation = ReconciliationContext {
                runtime: dependencies.runtime.as_ref(),
                ingress: dependencies.ingress.as_ref(),
                source_build: dependencies.source_build.as_ref(),
                dns_verifier: dependencies.dns_verifier.as_ref(),
            };
            match reconcile_runtime_state(
                &deployments,
                &domains,
                &cipher,
                &reconciliation,
                &publisher,
                &active_jobs,
                false,
            )
            .await
            {
                Ok(()) => worker_ready.store(true, Ordering::Release),
                Err(error) => {
                    worker_ready.store(false, Ordering::Release);
                    tracing::error!(error = %error, "deployment worker reconciliation failed");
                }
            }
            if let Err(error) = deployments.prune_retention().await {
                tracing::error!(error = %error, "deployment retention pruning failed");
            }
            while jobs.len() < MAX_CONCURRENT_DEPLOYMENT_JOBS {
                let Some(deployment) = (match deployments.claim_next().await {
                    Ok(deployment) => deployment,
                    Err(error) => {
                        tracing::error!(error = %error, "deployment worker could not claim deployment");
                        break;
                    }
                }) else {
                    break;
                };
                publisher
                    .publish_events(&deployments, deployment.id.as_str())
                    .await;
                let deployment_id = deployment.id.to_string();
                active_jobs.insert(deployment_id.clone());
                let task_deployments = deployments.clone();
                let task_domains = domains.clone();
                let task_cipher = cipher.clone();
                let task_runtime = dependencies.runtime.clone();
                let task_ingress = dependencies.ingress.clone();
                let task_source_build = dependencies.source_build.clone();
                let task_publisher = publisher.clone();
                jobs.spawn(async move {
                    let result = process_claimed_deployment(
                        &task_deployments,
                        &task_domains,
                        &task_cipher,
                        task_runtime.as_ref(),
                        task_ingress.as_ref(),
                        task_source_build.as_ref(),
                        &task_publisher,
                        deployment,
                    )
                    .await;
                    (deployment_id, result)
                });
            }
            tokio::select! {
                _ = interval.tick() => {}
                value = wake.recv() => {
                    if value.is_none() {
                        return;
                    }
                }
                Some(result) = jobs.join_next(), if !jobs.is_empty() => {
                    match result {
                        Ok((deployment_id, Ok(()))) => {
                            active_jobs.remove(&deployment_id);
                        }
                        Ok((deployment_id, Err(error))) => {
                            active_jobs.remove(&deployment_id);
                            tracing::error!(deployment_id = %deployment_id, error = %error, "deployment job failed");
                        }
                        Err(error) => {
                            tracing::error!(error = %error, "deployment job task failed");
                        }
                    }
                }
            }
        }
    });
    (handle, ready)
}
