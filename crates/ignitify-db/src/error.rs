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
}

pub type Result<T> = std::result::Result<T, DatabaseError>;
