//! Durable deployment submission, encrypted snapshots, and worker orchestration.

mod deployment_control;
mod deployment_retry;
mod deployment_stream;
mod deployment_values;
mod domain_dns;
mod health;
mod implementation;
mod model;
mod runtime;
mod service_control;
mod worker;

pub use deployment_control::{ControlHandle, DeploymentSubmission};
pub use deployment_stream::{DeploymentLogSink, StreamPublisher, StreamRecord};
pub use domain_dns::{
    DnsVerificationResult, DnsVerifier, NoopDnsVerifier, reconcile_dns_verifications,
};
pub use health::{
    HostRuntimeMetrics, RuntimeContainer, RuntimeHealth, RuntimePort, StaticRuntimeHealth,
    StaticSystemMetrics, SystemMetricsProvider, SystemMetricsSnapshot, WorkerHealth,
};
pub use implementation::{reconcile_once, reconcile_once_with_source};
pub use model::{
    AgeCipher, AutoDeploySecretRotation, AutoDeployWebhookTargetModel, Error,
    ProjectEnvironmentMutationModel, ProjectEnvironmentReadModel, ProjectEnvironmentVariableInput,
    ProjectEnvironmentVariableReadModel, Result, ServiceMutationOutcomeModel, ServiceReadModel,
    ServiceVariableReadModel,
};
pub use runtime::{
    ImageRuntime, Ingress, IngressRoute, NoRemoteRuntime, NoopSourceBuild, RuntimeDeployment,
    RuntimeLog, RuntimeObservation, RuntimeSelector, SourceBuild, SourceBuildOutput,
    WorkerDependencies,
};
pub use service_control::ServiceControl;
pub use worker::{spawn_worker, spawn_worker_with_source, spawn_worker_with_source_and_dns};
