use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error(transparent)]
    Operations(#[from] crate::operations::Error),
    #[error(transparent)]
    RuntimeSecrets(#[from] crate::runtime_secrets::Error),
    #[error(transparent)]
    Database(#[from] ignitify_db::DatabaseError),
    #[error(transparent)]
    Control(#[from] ignitify_control_plane::Error),
    #[error("Docker runtime unavailable")]
    DockerRuntime,
    #[error("invalid runtime configuration: {0}")]
    Configuration(&'static str),
    #[error(transparent)]
    ComposeRuntime(#[from] ignitify_runtime_compose::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
