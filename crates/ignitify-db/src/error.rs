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
    #[error("service name already exists")]
    ServiceNameConflict,
    #[error("registry name already exists")]
    RegistryNameConflict,
    #[error("webhook name already exists in project")]
    WebhookNameConflict,
    #[error("registry removal confirmation does not match name")]
    RegistryConfirmationMismatch,
    #[error("webhook removal confirmation does not match name")]
    WebhookConfirmationMismatch,
    #[error("invalid service kind: {0}")]
    InvalidServiceKind(String),
    #[error("invalid stored service specification: {0}")]
    InvalidServiceSpec(String),
    #[error("invalid stored deployment state: {0}")]
    InvalidDeploymentState(String),
    #[error("domain hostname already exists")]
    DomainNameConflict,
    #[error("domain removal confirmation does not match hostname")]
    DomainConfirmationMismatch,
    #[error("invalid stored domain status: {0}")]
    InvalidDomainStatus(String),
}

pub type Result<T> = std::result::Result<T, DatabaseError>;
