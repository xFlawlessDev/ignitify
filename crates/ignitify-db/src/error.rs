use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("invalid user role: {0}")]
    InvalidRole(String),
    #[error("invalid project membership role: {0}")]
    InvalidProjectMemberRole(String),
    #[error("project name already exists")]
    ProjectNameConflict,
    #[error("project removal confirmation does not match name")]
    ProjectConfirmationMismatch,
    #[error("project has an active deployment")]
    ProjectHasActiveDeployment,
    #[error("service name already exists")]
    ServiceNameConflict,
    #[error("service removal confirmation does not match name")]
    ServiceConfirmationMismatch,
    #[error("service has an active deployment")]
    ServiceHasActiveDeployment,
    #[error("invalid service kind: {0}")]
    InvalidServiceKind(String),
    #[error("invalid stored service specification: {0}")]
    InvalidServiceSpec(String),
    #[error("invalid stored service source configuration: {0}")]
    InvalidServiceSourceConfig(String),
    #[error("invalid stored deployment state: {0}")]
    InvalidDeploymentState(String),
    #[error("domain hostname already exists")]
    DomainNameConflict,
    #[error("domain removal confirmation does not match hostname")]
    DomainConfirmationMismatch,
    #[error("invalid stored domain status: {0}")]
    InvalidDomainStatus(String),
    #[error("invalid stored DNS record type: {0}")]
    InvalidDnsRecordType(String),
    #[error("invalid stored DNS record target: {0}")]
    InvalidDnsRecordTarget(String),
    #[error("invalid stored DNS verification status: {0}")]
    InvalidDnsVerificationStatus(String),
    #[error("provider name already exists")]
    ProviderNameConflict,
    #[error("remote builder name already exists")]
    RemoteBuilderNameConflict,
    #[error("invalid stored provider kind: {0}")]
    InvalidProviderKind(String),
    #[error("invalid stored provider auth mode: {0}")]
    InvalidProviderAuthMode(String),
    #[error("invalid concurrent build count")]
    InvalidConcurrentBuilds,
    #[error("invalid stored certificate provider: {0}")]
    InvalidCertificateProvider(String),
}

pub type Result<T> = std::result::Result<T, DatabaseError>;
