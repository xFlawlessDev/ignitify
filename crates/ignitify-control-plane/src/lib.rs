//! Durable deployment submission, encrypted snapshots, and worker orchestration.

use std::{
    collections::BTreeMap,
    future::Future,
    io::Write,
    pin::Pin,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use age::{Decryptor, Encryptor, x25519};
use ignitify_db::{
    AuthorizedDeploymentService, AuthorizedService, CreateDeploymentOutcome, DeploymentActor,
    DeploymentRecord, DeploymentsRepository, DomainsRepository, NewDeployment, NewServiceVariable,
    ServiceActor, ServiceMutationOutcome, ServicesRepository,
};
use ignitify_domain::{
    DeploymentState, DomainId, DomainName, ServiceId, ServiceInput, ServiceSpec,
};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc};
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct ServiceControl {
    cipher: Arc<AgeCipher>,
    services: ServicesRepository,
}

impl ServiceControl {
    pub fn new(services: ServicesRepository, identity: impl AsRef<str>) -> Result<Self> {
        Ok(Self {
            cipher: Arc::new(AgeCipher::from_identity(identity)?),
            services,
        })
    }

    pub async fn list(
        &self,
        actor: ServiceActor<'_>,
        project_id: &str,
    ) -> Result<Option<Vec<ServiceReadModel>>> {
        let services = self.services.list(actor, project_id).await?;
        services
            .map(|services| {
                services
                    .into_iter()
                    .map(|service| self.read_model(service))
                    .collect()
            })
            .transpose()
    }

    pub async fn get(
        &self,
        actor: ServiceActor<'_>,
        service_id: &str,
    ) -> Result<Option<ServiceReadModel>> {
        self.services
            .get(actor, service_id)
            .await?
            .map(|service| self.read_model(service))
            .transpose()
    }

    pub async fn create(
        &self,
        actor: ServiceActor<'_>,
        project_id: &str,
        input: ServiceInput,
    ) -> Result<ServiceMutationOutcomeModel> {
        let (configuration, variables) = self.encrypt_variables(input)?;
        Ok(
            match self
                .services
                .create(actor, project_id, configuration, variables)
                .await?
            {
                ServiceMutationOutcome::Created(service) => {
                    ServiceMutationOutcomeModel::Created(self.read_model(service)?)
                }
                ServiceMutationOutcome::Updated(_) => {
                    unreachable!("service create cannot return update")
                }
                ServiceMutationOutcome::Missing => ServiceMutationOutcomeModel::Missing,
                ServiceMutationOutcome::Forbidden => ServiceMutationOutcomeModel::Forbidden,
            },
        )
    }

    pub async fn update(
        &self,
        actor: ServiceActor<'_>,
        service_id: &str,
        input: ServiceInput,
    ) -> Result<ServiceMutationOutcomeModel> {
        let (configuration, variables) = self.encrypt_variables(input)?;
        Ok(
            match self
                .services
                .update(actor, service_id, configuration, variables)
                .await?
            {
                ServiceMutationOutcome::Created(_) => {
                    unreachable!("service update cannot return create")
                }
                ServiceMutationOutcome::Updated(service) => {
                    ServiceMutationOutcomeModel::Updated(self.read_model(service)?)
                }
                ServiceMutationOutcome::Missing => ServiceMutationOutcomeModel::Missing,
                ServiceMutationOutcome::Forbidden => ServiceMutationOutcomeModel::Forbidden,
            },
        )
    }

    fn encrypt_variables(
        &self,
        input: ServiceInput,
    ) -> Result<(
        ignitify_domain::ServiceConfiguration,
        Vec<NewServiceVariable>,
    )> {
        let variables = input
            .variables
            .into_iter()
            .map(|variable| {
                let plaintext = Zeroizing::new(variable.value);
                Ok(NewServiceVariable {
                    key: variable.key,
                    is_secret: variable.is_secret,
                    ciphertext: self.cipher.encrypt(plaintext.as_bytes())?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((input.configuration, variables))
    }

    fn read_model(&self, service: AuthorizedService) -> Result<ServiceReadModel> {
        let can_read_values = service.role.can_manage_services();
        let variables = service
            .variables
            .into_iter()
            .map(|variable| {
                let value = if variable.is_secret || !can_read_values {
                    None
                } else {
                    let plaintext = self.cipher.decrypt(&variable.ciphertext)?;
                    let value = std::str::from_utf8(plaintext.as_slice())
                        .map_err(|_| Error::InvalidCiphertext)?;
                    Some(Zeroizing::new(value.to_owned()))
                };
                Ok(ServiceVariableReadModel {
                    key: variable.key,
                    is_secret: variable.is_secret,
                    is_set: true,
                    value,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(ServiceReadModel {
            id: service.id.to_string(),
            project_id: service.project_id.to_string(),
            environment_id: service.environment_id.to_string(),
            role: service.role.as_str().to_owned(),
            name: service.name,
            kind: service.kind.as_str().to_owned(),
            spec: service.spec,
            desired_generation: service.desired_generation,
            desired_state: service.desired_state,
            created_at: service.created_at,
            updated_at: service.updated_at,
            variables,
        })
    }
}

#[derive(Clone)]
pub struct ControlHandle {
    cipher: Arc<AgeCipher>,
    deployments: DeploymentsRepository,
    wake: mpsc::Sender<()>,
    publisher: StreamPublisher,
}

impl ControlHandle {
    pub fn new(
        deployments: DeploymentsRepository,
        identity: impl AsRef<str>,
    ) -> Result<(Self, mpsc::Receiver<()>)> {
        let (wake, receiver) = mpsc::channel(16);
        let (stream, _) = broadcast::channel(256);
        Ok((
            Self {
                cipher: Arc::new(AgeCipher::from_identity(identity)?),
                deployments,
                wake,
                publisher: StreamPublisher::new(stream),
            },
            receiver,
        ))
    }

    pub async fn submit_deploy(
        &self,
        actor: DeploymentActor<'_>,
        service_id: &str,
        idempotency_key: &str,
    ) -> Result<DeploymentSubmission> {
        validate_idempotency_key(idempotency_key)?;
        let Some(service) = self
            .deployments
            .service_for_deployment(actor, service_id)
            .await?
        else {
            return Ok(DeploymentSubmission::Missing);
        };
        if !actor.is_admin && !service.role.can_manage_services() {
            return Ok(DeploymentSubmission::Forbidden);
        }
        let variables_ciphertext = self.snapshot_variables(&service)?;
        let spec = service.spec.clone();
        let outcome = self
            .deployments
            .create(
                actor,
                service_id,
                NewDeployment {
                    idempotency_key: idempotency_key.to_owned(),
                    requested_by_user_id: actor.id.to_owned(),
                    spec,
                    variables_ciphertext,
                },
            )
            .await?;
        if let CreateDeploymentOutcome::Created(record) = &outcome {
            self.publish_deployment_records(record.id.as_str()).await?;
            let _ = self.wake.try_send(());
        }
        Ok(match outcome {
            CreateDeploymentOutcome::Created(record) => DeploymentSubmission::Accepted(record),
            CreateDeploymentOutcome::Existing(record) => DeploymentSubmission::Existing(record),
            CreateDeploymentOutcome::Missing => DeploymentSubmission::Missing,
            CreateDeploymentOutcome::Forbidden => DeploymentSubmission::Forbidden,
            CreateDeploymentOutcome::ActiveConflict => DeploymentSubmission::ActiveConflict,
        })
    }

    pub async fn submit_rollback(
        &self,
        actor: DeploymentActor<'_>,
        deployment_id: &str,
        idempotency_key: &str,
    ) -> Result<DeploymentSubmission> {
        validate_idempotency_key(idempotency_key)?;
        let repository_key = format!("\u{1f}rollback:{deployment_id}:{idempotency_key}");
        let outcome = self
            .deployments
            .rollback(actor, deployment_id, &repository_key)
            .await?;
        if let CreateDeploymentOutcome::Created(record) = &outcome {
            self.publish_deployment_records(record.id.as_str()).await?;
            let _ = self.wake.try_send(());
        }
        Ok(match outcome {
            CreateDeploymentOutcome::Created(record) => DeploymentSubmission::Accepted(record),
            CreateDeploymentOutcome::Existing(record) => DeploymentSubmission::Existing(record),
            CreateDeploymentOutcome::Missing => DeploymentSubmission::Missing,
            CreateDeploymentOutcome::Forbidden => DeploymentSubmission::Forbidden,
            CreateDeploymentOutcome::ActiveConflict => DeploymentSubmission::ActiveConflict,
        })
    }

    pub async fn list(
        &self,
        actor: DeploymentActor<'_>,
        service_id: &str,
        before_created_at: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Option<Vec<DeploymentRecord>>> {
        Ok(self
            .deployments
            .list(actor, service_id, before_created_at, limit)
            .await?)
    }

    pub async fn list_for_project(
        &self,
        actor: DeploymentActor<'_>,
        project_id: &str,
        before_created_at: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Option<Vec<DeploymentRecord>>> {
        Ok(self
            .deployments
            .list_for_project(actor, project_id, before_created_at, limit)
            .await?)
    }

    pub fn worker_cipher(&self) -> Arc<AgeCipher> {
        self.cipher.clone()
    }

    pub fn wake_worker(&self) -> Result<()> {
        self.wake.try_send(()).map_err(|_| Error::WorkerUnavailable)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<StreamRecord> {
        self.publisher.sender.subscribe()
    }

    pub fn worker_publisher(&self) -> StreamPublisher {
        self.publisher.clone()
    }

    pub async fn events(
        &self,
        actor: DeploymentActor<'_>,
        deployment_id: &str,
        after: i64,
        through: i64,
    ) -> Result<Option<Vec<ignitify_db::DeploymentEventRecord>>> {
        if self.get(actor, deployment_id).await?.is_none() {
            return Ok(None);
        }
        Ok(Some(
            self.deployments
                .events_after(deployment_id, after, through)
                .await?,
        ))
    }

    pub async fn logs(
        &self,
        actor: DeploymentActor<'_>,
        deployment_id: &str,
        after: i64,
        through: i64,
    ) -> Result<Option<Vec<ignitify_db::DeploymentLogRecord>>> {
        if self.get(actor, deployment_id).await?.is_none() {
            return Ok(None);
        }
        Ok(Some(
            self.deployments
                .logs_after(deployment_id, after, through)
                .await?,
        ))
    }

    pub async fn event_cursor(
        &self,
        actor: DeploymentActor<'_>,
        deployment_id: &str,
    ) -> Result<Option<ignitify_db::SequenceCursor>> {
        if self.get(actor, deployment_id).await?.is_none() {
            return Ok(None);
        }
        Ok(Some(self.deployments.event_cursor(deployment_id).await?))
    }

    pub async fn log_cursor(
        &self,
        actor: DeploymentActor<'_>,
        deployment_id: &str,
    ) -> Result<Option<ignitify_db::SequenceCursor>> {
        if self.get(actor, deployment_id).await?.is_none() {
            return Ok(None);
        }
        Ok(Some(self.deployments.log_cursor(deployment_id).await?))
    }

    pub fn decrypt_variables(&self, ciphertext: &str) -> Result<Vec<String>> {
        decrypt_deployment_environment(&self.cipher, ciphertext)
    }

    pub async fn get(
        &self,
        actor: DeploymentActor<'_>,
        deployment_id: &str,
    ) -> Result<Option<DeploymentRecord>> {
        Ok(self.deployments.get(actor, deployment_id).await?)
    }

    pub async fn submit_stop(
        &self,
        actor: DeploymentActor<'_>,
        service_id: &str,
    ) -> Result<DeploymentSubmission> {
        let Some(service) = self
            .deployments
            .service_for_deployment(actor, service_id)
            .await?
        else {
            return Ok(DeploymentSubmission::Missing);
        };
        if !actor.is_admin && !service.role.can_manage_services() {
            return Ok(DeploymentSubmission::Forbidden);
        }
        let Some(mut deployment) = self.deployments.active_for_stop(actor, service_id).await?
        else {
            let Some(deployment) = self
                .deployments
                .list(actor, service_id, None, Some(1))
                .await?
                .and_then(|deployments| deployments.into_iter().next())
            else {
                return Ok(DeploymentSubmission::Missing);
            };
            return Ok(if deployment.state == DeploymentState::Stopping {
                DeploymentSubmission::Existing(deployment)
            } else {
                DeploymentSubmission::ActiveConflict
            });
        };
        if !self
            .deployments
            .transition(
                deployment.id.as_str(),
                DeploymentState::Stopping,
                deployment.runtime_ref.as_deref(),
                None,
            )
            .await?
        {
            return Ok(DeploymentSubmission::ActiveConflict);
        }
        self.publish_deployment_records(deployment.id.as_str())
            .await?;
        let _ = self.wake.try_send(());
        deployment.state = DeploymentState::Stopping;
        Ok(DeploymentSubmission::Accepted(deployment))
    }

    async fn publish_deployment_records(&self, deployment_id: &str) -> Result<()> {
        self.publisher
            .publish_events(&self.deployments, deployment_id)
            .await;
        Ok(())
    }

    fn snapshot_variables(&self, service: &AuthorizedDeploymentService) -> Result<String> {
        let mut values = BTreeMap::new();
        for variable in &service.variables {
            let plaintext = self.cipher.decrypt(&variable.ciphertext)?;
            let value = Zeroizing::new(
                String::from_utf8(plaintext.to_vec()).map_err(|_| Error::InvalidCiphertext)?,
            );
            values.insert(variable.key.clone(), value);
        }
        let plaintext =
            Zeroizing::new(serde_json::to_vec(&values).map_err(|_| Error::InvalidCiphertext)?);
        self.cipher.encrypt(plaintext.as_slice())
    }
}

#[derive(Clone)]
pub struct StreamPublisher {
    sender: broadcast::Sender<StreamRecord>,
}

impl StreamPublisher {
    fn new(sender: broadcast::Sender<StreamRecord>) -> Self {
        Self { sender }
    }

    async fn publish_events(&self, deployments: &DeploymentsRepository, deployment_id: &str) {
        if let Ok(events) = deployments.events(deployment_id).await {
            for event in events {
                let _ = self.sender.send(StreamRecord::Event(event));
            }
        }
    }

    fn publish_logs(&self, logs: Vec<ignitify_db::DeploymentLogRecord>) {
        for log in logs {
            let _ = self.sender.send(StreamRecord::Log(log));
        }
    }
}

#[derive(Debug, Clone)]
pub enum StreamRecord {
    Event(ignitify_db::DeploymentEventRecord),
    Log(ignitify_db::DeploymentLogRecord),
}

pub enum DeploymentSubmission {
    Accepted(DeploymentRecord),
    Existing(DeploymentRecord),
    Missing,
    Forbidden,
    ActiveConflict,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeObservation {
    pub owned: bool,
    pub running: bool,
    pub healthy: Option<bool>,
    pub health_failing: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct HostRuntimeMetrics {
    pub containers: i64,
    pub containers_running: i64,
    pub images: i64,
    pub cpus: i64,
    pub memory_bytes: i64,
}

pub trait RuntimeHealth: Send + Sync {
    fn ready(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;

    fn host_metrics(
        &self,
    ) -> Pin<Box<dyn Future<Output = Option<HostRuntimeMetrics>> + Send + '_>> {
        Box::pin(std::future::ready(None))
    }
}

pub struct StaticRuntimeHealth(pub bool);

impl RuntimeHealth for StaticRuntimeHealth {
    fn ready(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(std::future::ready(self.0))
    }
}

pub struct WorkerHealth(pub Arc<AtomicBool>);

impl RuntimeHealth for WorkerHealth {
    fn ready(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(std::future::ready(self.0.load(Ordering::Acquire)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngressRoute {
    pub labels: BTreeMap<String, String>,
    pub network: String,
}

pub trait Ingress: Send + Sync + 'static {
    fn route(
        &self,
        service_id: &ServiceId,
        domain_id: &DomainId,
        hostname: &DomainName,
        port: u32,
    ) -> Result<IngressRoute>;
}

pub trait ImageRuntime: Send + Sync + 'static {
    fn runtime_ref(&self, deployment: &DeploymentRecord) -> String;

    fn start(
        &self,
        deployment: &DeploymentRecord,
        environment: Vec<String>,
    ) -> impl std::future::Future<Output = Result<String>> + Send;

    fn inspect(
        &self,
        deployment: &DeploymentRecord,
        runtime_ref: &str,
    ) -> impl std::future::Future<Output = Result<RuntimeObservation>> + Send;

    fn stop(
        &self,
        runtime_ref: &str,
        service_id: &str,
        generation: i64,
    ) -> impl std::future::Future<Output = Result<bool>> + Send;

    fn logs(
        &self,
        runtime_ref: &str,
        since: i64,
    ) -> impl std::future::Future<Output = Result<Vec<ignitify_db::NewDeploymentLog>>> + Send;

    fn reconcile_routes(
        &self,
        _deployment: &DeploymentRecord,
        _environment: Vec<String>,
        _routes: Vec<IngressRoute>,
    ) -> impl std::future::Future<Output = Result<bool>> + Send {
        async { Ok(true) }
    }
}

#[derive(Clone)]
pub struct RuntimeSelector<I, C> {
    image: I,
    compose: C,
}

impl<I, C> RuntimeSelector<I, C> {
    pub fn new(image: I, compose: C) -> Self {
        Self { image, compose }
    }
}

impl<I, C> RuntimeHealth for RuntimeSelector<I, C>
where
    I: RuntimeHealth + Send + Sync,
    C: RuntimeHealth + Send + Sync,
{
    fn ready(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async move { self.image.ready().await && self.compose.ready().await })
    }

    fn host_metrics(
        &self,
    ) -> Pin<Box<dyn Future<Output = Option<HostRuntimeMetrics>> + Send + '_>> {
        self.image.host_metrics()
    }
}

impl<I, C> ImageRuntime for RuntimeSelector<I, C>
where
    I: ImageRuntime,
    C: ImageRuntime,
{
    fn runtime_ref(&self, deployment: &DeploymentRecord) -> String {
        match &deployment.spec {
            ServiceSpec::Image { .. } => self.image.runtime_ref(deployment),
            ServiceSpec::Compose { .. } => self.compose.runtime_ref(deployment),
        }
    }

    async fn start(
        &self,
        deployment: &DeploymentRecord,
        environment: Vec<String>,
    ) -> Result<String> {
        match &deployment.spec {
            ServiceSpec::Image { .. } => self.image.start(deployment, environment).await,
            ServiceSpec::Compose { .. } => self.compose.start(deployment, environment).await,
        }
    }

    async fn inspect(
        &self,
        deployment: &DeploymentRecord,
        runtime_ref: &str,
    ) -> Result<RuntimeObservation> {
        match &deployment.spec {
            ServiceSpec::Image { .. } => self.image.inspect(deployment, runtime_ref).await,
            ServiceSpec::Compose { .. } => self.compose.inspect(deployment, runtime_ref).await,
        }
    }

    async fn stop(&self, runtime_ref: &str, service_id: &str, generation: i64) -> Result<bool> {
        if runtime_ref.starts_with("ignitify-svc-") {
            self.image.stop(runtime_ref, service_id, generation).await
        } else {
            self.compose.stop(runtime_ref, service_id, generation).await
        }
    }

    async fn logs(
        &self,
        runtime_ref: &str,
        since: i64,
    ) -> Result<Vec<ignitify_db::NewDeploymentLog>> {
        if runtime_ref.starts_with("ignitify-svc-") {
            self.image.logs(runtime_ref, since).await
        } else {
            self.compose.logs(runtime_ref, since).await
        }
    }

    async fn reconcile_routes(
        &self,
        deployment: &DeploymentRecord,
        environment: Vec<String>,
        routes: Vec<IngressRoute>,
    ) -> Result<bool> {
        match &deployment.spec {
            ServiceSpec::Image { .. } => {
                self.image
                    .reconcile_routes(deployment, environment, routes)
                    .await
            }
            ServiceSpec::Compose { .. } => {
                self.compose
                    .reconcile_routes(deployment, environment, routes)
                    .await
            }
        }
    }
}

pub fn spawn_worker<R, I>(
    deployments: DeploymentsRepository,
    domains: DomainsRepository,
    cipher: Arc<AgeCipher>,
    runtime: R,
    ingress: I,
    publisher: StreamPublisher,
    mut wake: mpsc::Receiver<()>,
) -> (tokio::task::JoinHandle<()>, Arc<AtomicBool>)
where
    R: ImageRuntime,
    I: Ingress,
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
        loop {
            match reconcile_once(
                &deployments,
                &domains,
                &cipher,
                &runtime,
                &ingress,
                &publisher,
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
            tokio::select! {
                _ = interval.tick() => {}
                value = wake.recv() => {
                    if value.is_none() {
                        return;
                    }
                }
            }
        }
    });
    (handle, ready)
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
    for deployment in deployments.nonterminal().await? {
        match deployment.state {
            DeploymentState::Queued => {}
            DeploymentState::Preparing | DeploymentState::Running => {
                if let Some(runtime_ref) = deployment.runtime_ref.as_deref() {
                    let observation = runtime.inspect(&deployment, runtime_ref).await?;
                    let became_healthy = advance_observed_deployment(
                        deployments,
                        runtime,
                        &deployment,
                        runtime_ref,
                        observation,
                        publisher,
                    )
                    .await?;
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
                    if became_healthy {
                        cleanup_prior_deployments(deployments, runtime, &deployment, publisher)
                            .await?;
                    }
                } else if deployment.state == DeploymentState::Preparing {
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
        reconcile_routes(domains, cipher, runtime, ingress, &deployment).await?;
    }
    let Some(deployment) = deployments.claim_next().await? else {
        return Ok(());
    };
    publisher
        .publish_events(deployments, deployment.id.as_str())
        .await;
    let predicted_runtime_ref = runtime.runtime_ref(&deployment);
    deployments
        .record_runtime_ref(deployment.id.as_str(), &predicted_runtime_ref)
        .await?;
    let environment = decrypt_deployment_environment(cipher, &deployment.variables_ciphertext)?;
    let runtime_ref = match runtime.start(&deployment, environment).await {
        Ok(runtime_ref) => {
            deployments
                .replace_runtime_ref(deployment.id.as_str(), &runtime_ref)
                .await?;
            runtime_ref
        }
        Err(Error::Policy(_)) => {
            deployments
                .transition(
                    deployment.id.as_str(),
                    DeploymentState::Failed,
                    Some(&predicted_runtime_ref),
                    Some("runtime policy rejected input"),
                )
                .await?;
            publisher
                .publish_events(deployments, deployment.id.as_str())
                .await;
            return Ok(());
        }
        Err(error) => {
            tracing::warn!(deployment_id = %deployment.id, error = %error, "deployment runtime start uncertain");
            predicted_runtime_ref
        }
    };
    let observation = match runtime.inspect(&deployment, &runtime_ref).await {
        Ok(observation) => observation,
        Err(error) => {
            tracing::warn!(deployment_id = %deployment.id, error = %error, "deployment runtime inspection uncertain");
            return Ok(());
        }
    };
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
        persist_logs(
            deployments,
            cipher,
            runtime,
            &deployment,
            &runtime_ref,
            publisher,
        )
        .await?;
        reconcile_routes(domains, cipher, runtime, ingress, &deployment).await?;
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
    let logs = redact_logs(runtime.logs(runtime_ref, since).await?, &values);
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
) -> Result<()>
where
    R: ImageRuntime,
    I: Ingress,
{
    let Some(port) = deployment.spec.internal_port() else {
        return Ok(());
    };
    let domain_records = domains
        .active_for_service(deployment.service_id.as_str())
        .await?;
    let mut routes = Vec::with_capacity(domain_records.len());
    for domain in &domain_records {
        routes.push(ingress.route(&deployment.service_id, &domain.id, &domain.hostname, port)?);
    }
    let environment = decrypt_deployment_environment(cipher, &deployment.variables_ciphertext)?;
    let applied = runtime
        .reconcile_routes(deployment, environment, routes)
        .await?;
    for domain in domain_records {
        domains
            .set_status(
                domain.id.as_str(),
                if applied {
                    ignitify_domain::DomainStatus::Active
                } else {
                    ignitify_domain::DomainStatus::Failed
                },
                (!applied).then_some("route reconciliation failed"),
            )
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

fn decrypt_deployment_environment(cipher: &AgeCipher, ciphertext: &str) -> Result<Vec<String>> {
    Ok(decrypt_deployment_values(cipher, ciphertext)?
        .into_iter()
        .map(|(key, value)| format!("{key}={}", value.as_str()))
        .collect())
}

fn deployment_secret_values(
    cipher: &AgeCipher,
    ciphertext: &str,
) -> Result<Vec<Zeroizing<String>>> {
    Ok(decrypt_deployment_values(cipher, ciphertext)?
        .into_values()
        .filter(|value| !value.is_empty())
        .collect())
}

fn redact_logs(
    logs: Vec<ignitify_db::NewDeploymentLog>,
    values: &[Zeroizing<String>],
) -> Vec<ignitify_db::NewDeploymentLog> {
    if values.is_empty() {
        return logs;
    }
    logs.into_iter()
        .map(|mut log| {
            log.line = "[REDACTED]".to_owned();
            log
        })
        .collect()
}

fn decrypt_deployment_values(
    cipher: &AgeCipher,
    ciphertext: &str,
) -> Result<BTreeMap<String, Zeroizing<String>>> {
    let plaintext = cipher.decrypt(ciphertext)?;
    serde_json::from_slice(plaintext.as_slice()).map_err(|_| Error::InvalidCiphertext)
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

pub struct AgeCipher {
    identity: x25519::Identity,
    recipient: x25519::Recipient,
}

impl AgeCipher {
    pub fn from_identity(identity: impl AsRef<str>) -> Result<Self> {
        let identity =
            x25519::Identity::from_str(identity.as_ref()).map_err(|_| Error::InvalidIdentity)?;
        let recipient = identity.to_public();
        Ok(Self {
            identity,
            recipient,
        })
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<String> {
        let encryptor =
            Encryptor::with_recipients(std::iter::once(&self.recipient as &dyn age::Recipient))
                .map_err(|_| Error::Encryption)?;
        let mut output = Vec::new();
        {
            let armor =
                age::armor::ArmoredWriter::wrap_output(&mut output, age::armor::Format::AsciiArmor)
                    .map_err(|_| Error::Encryption)?;
            let mut writer = encryptor
                .wrap_output(armor)
                .map_err(|_| Error::Encryption)?;
            writer.write_all(plaintext).map_err(|_| Error::Encryption)?;
            writer
                .finish()
                .map_err(|_| Error::Encryption)?
                .finish()
                .map_err(|_| Error::Encryption)?;
        }
        String::from_utf8(output).map_err(|_| Error::Encryption)
    }

    pub fn decrypt(&self, ciphertext: &str) -> Result<Zeroizing<Vec<u8>>> {
        let decryptor = Decryptor::new(age::armor::ArmoredReader::new(ciphertext.as_bytes()))
            .map_err(|_| Error::InvalidCiphertext)?;
        let mut reader = decryptor
            .decrypt(std::iter::once(&self.identity as &dyn age::Identity))
            .map_err(|_| Error::InvalidCiphertext)?;
        let mut plaintext = Zeroizing::new(Vec::new());
        std::io::Read::read_to_end(&mut reader, &mut plaintext)
            .map_err(|_| Error::InvalidCiphertext)?;
        Ok(plaintext)
    }
}

#[derive(Debug, Clone)]
pub struct ServiceReadModel {
    pub id: String,
    pub project_id: String,
    pub environment_id: String,
    pub role: String,
    pub name: String,
    pub kind: String,
    pub spec: ignitify_domain::ServiceSpec,
    pub desired_generation: i64,
    pub desired_state: String,
    pub created_at: String,
    pub updated_at: String,
    pub variables: Vec<ServiceVariableReadModel>,
}

#[derive(Debug, Clone)]
pub struct ServiceVariableReadModel {
    pub key: String,
    pub is_secret: bool,
    pub is_set: bool,
    pub value: Option<Zeroizing<String>>,
}

#[derive(Debug, Clone)]
pub enum ServiceMutationOutcomeModel {
    Created(ServiceReadModel),
    Updated(ServiceReadModel),
    Missing,
    Forbidden,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid age identity")]
    InvalidIdentity,
    #[error("could not encrypt service variable")]
    Encryption,
    #[error("invalid encrypted service variable")]
    InvalidCiphertext,
    #[error("idempotency key must use visible ASCII and be 1 to 128 bytes")]
    InvalidIdempotencyKey,
    #[error("image runtime failed")]
    Runtime,
    #[error("runtime policy rejected input: {0}")]
    Policy(&'static str),
    #[error("worker is unavailable")]
    WorkerUnavailable,
    #[error(transparent)]
    Database(#[from] ignitify_db::DatabaseError),
}

pub type Result<T> = std::result::Result<T, Error>;

fn validate_idempotency_key(value: &str) -> Result<()> {
    if !(1..=128).contains(&value.len()) || value.bytes().any(|byte| !(0x21..=0x7e).contains(&byte))
    {
        return Err(Error::InvalidIdempotencyKey);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use age::secrecy::ExposeSecret;
    use ignitify_db::{
        Database, DatabaseConfig, DeploymentActor, NewServiceVariable, ServiceActor,
        UserRole as DatabaseUserRole,
    };
    use ignitify_domain::{ProjectInput, ServiceInput};

    use super::{AgeCipher, ImageRuntime, Ingress, IngressRoute, reconcile_once};

    #[test]
    fn deployment_logs_redact_snapshot_values() {
        let logs = super::redact_logs(
            vec![ignitify_db::NewDeploymentLog {
                stream: "stdout".to_owned(),
                line: "TOKEN=plain-secret".to_owned(),
            }],
            &[zeroize::Zeroizing::new("plain-secret".to_owned())],
        );

        assert_eq!(logs[0].line, "[REDACTED]");
    }

    #[test]
    fn deployment_logs_redact_when_any_variable_is_present() {
        let logs = super::redact_logs(
            vec![ignitify_db::NewDeploymentLog {
                stream: "stdout".to_owned(),
                line: "safe output".to_owned(),
            }],
            &[zeroize::Zeroizing::new("plain-secret".to_owned())],
        );

        assert_eq!(logs[0].line, "[REDACTED]");
    }

    #[test]
    fn deployment_logs_redact_multiline_snapshot_values() {
        let logs = super::redact_logs(
            vec![
                ignitify_db::NewDeploymentLog {
                    stream: "stdout".to_owned(),
                    line: "TOKEN=first".to_owned(),
                },
                ignitify_db::NewDeploymentLog {
                    stream: "stdout".to_owned(),
                    line: "second".to_owned(),
                },
            ],
            &[zeroize::Zeroizing::new("first\nsecond".to_owned())],
        );

        assert_eq!(
            logs.into_iter().map(|log| log.line).collect::<Vec<_>>(),
            ["[REDACTED]", "[REDACTED]"]
        );
    }

    #[test]
    fn ciphertext_excludes_plaintext() {
        let identity = age::x25519::Identity::generate();
        let identity = identity.to_string();
        let cipher = AgeCipher::from_identity(identity.expose_secret()).unwrap();
        let ciphertext = cipher.encrypt(b"not-in-ciphertext").unwrap();

        assert!(!ciphertext.contains("not-in-ciphertext"));
        assert_eq!(
            cipher.decrypt(&ciphertext).unwrap().as_slice(),
            b"not-in-ciphertext"
        );
    }

    struct FakeIngress;

    impl Ingress for FakeIngress {
        fn route(
            &self,
            _service_id: &ignitify_domain::ServiceId,
            _domain_id: &ignitify_domain::DomainId,
            _hostname: &ignitify_domain::DomainName,
            _port: u32,
        ) -> super::Result<IngressRoute> {
            Ok(IngressRoute {
                labels: std::collections::BTreeMap::new(),
                network: "none".to_owned(),
            })
        }
    }

    struct FakeRuntime {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl ImageRuntime for FakeRuntime {
        fn runtime_ref(&self, deployment: &ignitify_db::DeploymentRecord) -> String {
            format!("runtime-{}", deployment.id)
        }

        async fn start(
            &self,
            deployment: &ignitify_db::DeploymentRecord,
            _environment: Vec<String>,
        ) -> super::Result<String> {
            self.calls.lock().unwrap().push(deployment.id.to_string());
            Ok(format!("runtime-{}", deployment.id))
        }

        async fn inspect(
            &self,
            _deployment: &ignitify_db::DeploymentRecord,
            _runtime_ref: &str,
        ) -> super::Result<super::RuntimeObservation> {
            Ok(super::RuntimeObservation {
                owned: true,
                running: true,
                healthy: Some(false),
                health_failing: false,
            })
        }

        async fn stop(
            &self,
            _runtime_ref: &str,
            _service_id: &str,
            _generation: i64,
        ) -> super::Result<bool> {
            Ok(true)
        }

        async fn logs(
            &self,
            _runtime_ref: &str,
            _since: i64,
        ) -> super::Result<Vec<ignitify_db::NewDeploymentLog>> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn worker_restart_scan_recovers_preparing_deployment_without_restarting_runtime() {
        let database = Database::connect(&DatabaseConfig {
            url: "sqlite::memory:".to_owned(),
        })
        .await
        .unwrap();
        let actor_id = database
            .users()
            .create("owner", "hash", DatabaseUserRole::User)
            .await
            .unwrap()
            .id;
        let project = database
            .projects()
            .create(&actor_id, ProjectInput::new("Platform").unwrap())
            .await
            .unwrap();
        let input =
            ServiceInput::image("web", "nginx@sha256:deadbeef", Some(8080), None, vec![]).unwrap();
        let service = database
            .services()
            .create(
                ServiceActor {
                    id: &actor_id,
                    is_admin: false,
                },
                project.id.as_str(),
                input.configuration,
                Vec::<NewServiceVariable>::new(),
            )
            .await
            .unwrap();
        let ignitify_db::ServiceMutationOutcome::Created(service) = service else {
            panic!("service must exist");
        };
        let identity = age::x25519::Identity::generate().to_string();
        let cipher = AgeCipher::from_identity(identity.expose_secret()).unwrap();
        let variables_ciphertext = cipher.encrypt(b"{}").unwrap();
        let deployment = database
            .deployments()
            .create(
                DeploymentActor {
                    id: &actor_id,
                    is_admin: false,
                },
                service.id.as_str(),
                ignitify_db::NewDeployment {
                    idempotency_key: "deploy-1".to_owned(),
                    requested_by_user_id: actor_id.clone(),
                    spec: service.spec,
                    variables_ciphertext,
                },
            )
            .await
            .unwrap();
        let ignitify_db::CreateDeploymentOutcome::Created(deployment) = deployment else {
            panic!("deployment must be created");
        };
        let claimed = database.deployments().claim_next().await.unwrap().unwrap();
        database
            .deployments()
            .record_runtime_ref(claimed.id.as_str(), &format!("runtime-{}", claimed.id))
            .await
            .unwrap();
        let runtime = FakeRuntime {
            calls: Arc::new(Mutex::new(vec![])),
        };
        let (publisher, _) = tokio::sync::broadcast::channel(16);
        reconcile_once(
            &database.deployments(),
            &database.domains(),
            &cipher,
            &runtime,
            &FakeIngress,
            &super::StreamPublisher::new(publisher),
        )
        .await
        .unwrap();

        let events = database
            .deployments()
            .events(deployment.id.as_str())
            .await
            .unwrap();
        assert_eq!(
            events
                .into_iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            [
                "deployment.queued",
                "deployment.preparing",
                "deployment.running",
                "deployment.healthy"
            ]
        );
        assert!(runtime.calls.lock().unwrap().is_empty());
    }
}
