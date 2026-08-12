use std::{collections::BTreeMap, future::Future, sync::Arc};

use ignitify_db::DeploymentRecord;
use ignitify_domain::{DomainId, DomainName, ServiceId, ServiceSpec};

use crate::{
    DeploymentLogSink, Error, HostRuntimeMetrics, NoopDnsVerifier, Result, RuntimeContainer,
    RuntimeHealth,
};

/// Runtime-only deployment data. Adapters never receive persistence records or ciphertext.
#[derive(Debug, Clone)]
pub struct RuntimeDeployment {
    pub id: ignitify_domain::DeploymentId,
    pub service_id: ServiceId,
    pub generation: i64,
    pub spec: ServiceSpec,
    pub local_image_id: Option<String>,
    pub deployment_destination_id: Option<String>,
}

impl From<&DeploymentRecord> for RuntimeDeployment {
    fn from(deployment: &DeploymentRecord) -> Self {
        Self {
            id: deployment.id.clone(),
            service_id: deployment.service_id.clone(),
            generation: deployment.generation,
            spec: deployment.spec.clone(),
            local_image_id: deployment.local_image_id.clone(),
            deployment_destination_id: deployment.deployment_destination_id.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeLog {
    pub stream: String,
    pub line: String,
}

impl From<RuntimeLog> for ignitify_db::NewDeploymentLog {
    fn from(log: RuntimeLog) -> Self {
        Self {
            stream: log.stream,
            line: log.line,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeObservation {
    pub owned: bool,
    pub running: bool,
    pub healthy: Option<bool>,
    pub health_failing: bool,
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

    fn reconcile(&self) -> impl Future<Output = Result<()>> + Send {
        async { Ok(()) }
    }

    fn ensure_ready(&self) -> impl Future<Output = Result<bool>> + Send {
        async { Ok(true) }
    }
}

pub trait ImageRuntime: Send + Sync + 'static {
    fn runtime_ref(&self, deployment: &RuntimeDeployment) -> String;

    fn start(
        &self,
        deployment: &RuntimeDeployment,
        environment: Vec<String>,
    ) -> impl Future<Output = Result<String>> + Send;

    fn inspect(
        &self,
        deployment: &RuntimeDeployment,
        runtime_ref: &str,
    ) -> impl Future<Output = Result<RuntimeObservation>> + Send;

    fn stop(
        &self,
        runtime_ref: &str,
        service_id: &str,
        generation: i64,
    ) -> impl Future<Output = Result<bool>> + Send;

    fn logs(
        &self,
        runtime_ref: &str,
        since: i64,
    ) -> impl Future<Output = Result<Vec<RuntimeLog>>> + Send;

    fn reconcile_routes(
        &self,
        deployment: &RuntimeDeployment,
        runtime_ref: &str,
        environment: Vec<String>,
        routes: Vec<IngressRoute>,
    ) -> impl Future<Output = Result<bool>> + Send {
        let _ = (deployment, runtime_ref, environment, routes);
        async { Ok(true) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBuildOutput {
    pub source_revision: String,
    pub local_image_id: Option<String>,
    pub runtime_spec: Option<ServiceSpec>,
}

pub trait SourceBuild: Send + Sync + 'static {
    fn build(
        &self,
        deployment: &DeploymentRecord,
        logs: &DeploymentLogSink,
    ) -> impl Future<Output = Result<Option<SourceBuildOutput>>> + Send;
}

#[derive(Clone, Copy, Default)]
pub struct NoopSourceBuild;

impl SourceBuild for NoopSourceBuild {
    async fn build(
        &self,
        deployment: &DeploymentRecord,
        _logs: &DeploymentLogSink,
    ) -> Result<Option<SourceBuildOutput>> {
        if deployment.source_config.as_ref().is_some_and(|source| {
            source.source == "application"
                || (source.source == "compose" && source.provider_id.is_some())
        }) {
            return Err(Error::Policy("source builds are unavailable"));
        }
        Ok(None)
    }
}

pub struct WorkerDependencies<R, I, S, V = NoopDnsVerifier> {
    pub(crate) runtime: Arc<R>,
    pub(crate) ingress: Arc<I>,
    pub(crate) source_build: Arc<S>,
    pub(crate) dns_verifier: Arc<V>,
}

impl<R, I, S> WorkerDependencies<R, I, S> {
    pub fn new(runtime: R, ingress: I, source_build: S) -> Self {
        Self {
            runtime: Arc::new(runtime),
            ingress: Arc::new(ingress),
            source_build: Arc::new(source_build),
            dns_verifier: Arc::new(NoopDnsVerifier),
        }
    }
}

impl<R, I, S, V> WorkerDependencies<R, I, S, V> {
    pub fn with_dns_verifier<V2>(self, dns_verifier: V2) -> WorkerDependencies<R, I, S, V2> {
        WorkerDependencies {
            runtime: self.runtime,
            ingress: self.ingress,
            source_build: self.source_build,
            dns_verifier: Arc::new(dns_verifier),
        }
    }
}

#[derive(Clone)]
pub struct RuntimeSelector<I, C, R = NoRemoteRuntime> {
    image: I,
    compose: C,
    remote: R,
}

impl<I, C> RuntimeSelector<I, C, NoRemoteRuntime> {
    pub fn new(image: I, compose: C) -> Self {
        Self {
            image,
            compose,
            remote: NoRemoteRuntime,
        }
    }

    pub fn with_remote<R>(self, remote: R) -> RuntimeSelector<I, C, R> {
        RuntimeSelector {
            image: self.image,
            compose: self.compose,
            remote,
        }
    }
}

impl<I, C, R> RuntimeHealth for RuntimeSelector<I, C, R>
where
    I: RuntimeHealth + Send + Sync,
    C: RuntimeHealth + Send + Sync,
    R: Send + Sync,
{
    fn ready(&self) -> std::pin::Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async move { self.image.ready().await && self.compose.ready().await })
    }

    fn host_metrics(
        &self,
    ) -> std::pin::Pin<Box<dyn Future<Output = Option<HostRuntimeMetrics>> + Send + '_>> {
        self.image.host_metrics()
    }

    fn container_inventory(
        &self,
    ) -> std::pin::Pin<Box<dyn Future<Output = Option<Vec<RuntimeContainer>>> + Send + '_>> {
        self.image.container_inventory()
    }
}

impl<I, C, R> ImageRuntime for RuntimeSelector<I, C, R>
where
    I: ImageRuntime,
    C: ImageRuntime,
    R: ImageRuntime,
{
    fn runtime_ref(&self, deployment: &RuntimeDeployment) -> String {
        if deployment.deployment_destination_id.is_some() {
            return self.remote.runtime_ref(deployment);
        }
        match &deployment.spec {
            ServiceSpec::Image { .. } => self.image.runtime_ref(deployment),
            ServiceSpec::Compose { .. } => self.compose.runtime_ref(deployment),
        }
    }

    async fn start(
        &self,
        deployment: &RuntimeDeployment,
        environment: Vec<String>,
    ) -> Result<String> {
        if deployment.deployment_destination_id.is_some() {
            return self.remote.start(deployment, environment).await;
        }
        match &deployment.spec {
            ServiceSpec::Image { .. } => self.image.start(deployment, environment).await,
            ServiceSpec::Compose { .. } => self.compose.start(deployment, environment).await,
        }
    }

    async fn inspect(
        &self,
        deployment: &RuntimeDeployment,
        runtime_ref: &str,
    ) -> Result<RuntimeObservation> {
        if deployment.deployment_destination_id.is_some() {
            return self.remote.inspect(deployment, runtime_ref).await;
        }
        match &deployment.spec {
            ServiceSpec::Image { .. } => self.image.inspect(deployment, runtime_ref).await,
            ServiceSpec::Compose { .. } => self.compose.inspect(deployment, runtime_ref).await,
        }
    }

    async fn stop(&self, runtime_ref: &str, service_id: &str, generation: i64) -> Result<bool> {
        if runtime_ref.starts_with("ignitify-remote-") {
            return self.remote.stop(runtime_ref, service_id, generation).await;
        }
        if runtime_ref.starts_with("ignitify-svc-") {
            self.image.stop(runtime_ref, service_id, generation).await
        } else {
            self.compose.stop(runtime_ref, service_id, generation).await
        }
    }

    async fn logs(&self, runtime_ref: &str, since: i64) -> Result<Vec<RuntimeLog>> {
        if runtime_ref.starts_with("ignitify-remote-") {
            return self.remote.logs(runtime_ref, since).await;
        }
        if runtime_ref.starts_with("ignitify-svc-") {
            self.image.logs(runtime_ref, since).await
        } else {
            self.compose.logs(runtime_ref, since).await
        }
    }

    async fn reconcile_routes(
        &self,
        deployment: &RuntimeDeployment,
        runtime_ref: &str,
        environment: Vec<String>,
        routes: Vec<IngressRoute>,
    ) -> Result<bool> {
        if deployment.deployment_destination_id.is_some() {
            return self
                .remote
                .reconcile_routes(deployment, runtime_ref, environment, routes)
                .await;
        }
        match &deployment.spec {
            ServiceSpec::Image { .. } => {
                self.image
                    .reconcile_routes(deployment, runtime_ref, environment, routes)
                    .await
            }
            ServiceSpec::Compose { .. } => {
                self.compose
                    .reconcile_routes(deployment, runtime_ref, environment, routes)
                    .await
            }
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct NoRemoteRuntime;

impl ImageRuntime for NoRemoteRuntime {
    fn runtime_ref(&self, deployment: &RuntimeDeployment) -> String {
        format!("ignitify-remote-unavailable-{}", deployment.id)
    }

    async fn start(
        &self,
        _deployment: &RuntimeDeployment,
        _environment: Vec<String>,
    ) -> Result<String> {
        Err(Error::Policy("remote deployment runtime is unavailable"))
    }

    async fn inspect(
        &self,
        _deployment: &RuntimeDeployment,
        _runtime_ref: &str,
    ) -> Result<RuntimeObservation> {
        Err(Error::Runtime)
    }

    async fn stop(&self, _runtime_ref: &str, _service_id: &str, _generation: i64) -> Result<bool> {
        Err(Error::Runtime)
    }

    async fn logs(&self, _runtime_ref: &str, _since: i64) -> Result<Vec<RuntimeLog>> {
        Err(Error::Runtime)
    }
}
