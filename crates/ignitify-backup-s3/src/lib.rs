//! S3-compatible storage for durable Ignitify backup snapshots.

mod error;
mod upload;

pub use error::{Error, Result};
pub use upload::{BackupS3Destination, BackupS3DestinationInput, BackupUpload, upload_backup};
