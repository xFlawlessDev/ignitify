use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("backup input file is missing or unreadable: {0}")]
    ReadFile(PathBuf, #[source] std::io::Error),
    #[error("backup manifest could not be encoded")]
    Manifest(#[source] serde_json::Error),
    #[error("S3 request could not be signed")]
    Signing,
    #[error("S3 endpoint is invalid")]
    Endpoint,
    #[error("S3 object key is invalid")]
    ObjectKey,
    #[error("S3 rejected the backup upload with HTTP status {0}")]
    RemoteStatus(u16),
    #[error("S3 backup upload failed")]
    Http(#[source] reqwest::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
