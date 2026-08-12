#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Docker executable path must be absolute")]
    InvalidDockerPath,
    #[error("Compose specification is not supported by this runtime")]
    UnsupportedSpec,
    #[error("Compose policy rejected input: {0}")]
    Policy(&'static str),
    #[error("Docker Compose command failed: {0}")]
    CommandFailed(String),
    #[error("Docker Compose returned invalid canonical configuration")]
    InvalidCanonicalConfig,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
