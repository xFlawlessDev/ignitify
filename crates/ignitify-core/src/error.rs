use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("missing required environment variable: {0}")]
    MissingEnvironment(&'static str),
    #[error(transparent)]
    Database(#[from] ignitify_db::DatabaseError),
    #[error(transparent)]
    Control(#[from] ignitify_control_plane::Error),
    #[error("Docker runtime unavailable")]
    DockerRuntime,
    #[error(transparent)]
    ComposeRuntime(#[from] ignitify_runtime_compose::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
