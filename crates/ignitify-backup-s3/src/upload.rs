use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit, Mac};
use reqwest::{
    Client, StatusCode,
    header::{CONTENT_LENGTH, CONTENT_TYPE},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::{fs::File, io::AsyncReadExt, time::sleep};
use tokio_util::io::ReaderStream;
use url::Url;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{Error, Result};

const DATABASE_FILE_NAME: &str = "ignitify.db";
const SECRETS_FILE_NAME: &str = "ignitify-secrets.json";
const MANIFEST_FILE_NAME: &str = "manifest.json";
const UPLOAD_ATTEMPTS: usize = 3;

type HmacSha256 = Hmac<Sha256>;

pub struct BackupS3Destination {
    endpoint: Url,
    region: String,
    bucket: String,
    prefix: String,
    access_key_id: Zeroizing<String>,
    secret_access_key: Zeroizing<String>,
    session_token: Option<Zeroizing<String>>,
    server_side_encryption: String,
}

pub struct BackupS3DestinationInput {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub prefix: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
    pub server_side_encryption: String,
}

impl BackupS3Destination {
    pub fn new(input: BackupS3DestinationInput) -> Result<Self> {
        let endpoint = Url::parse(&input.endpoint).map_err(|_| Error::Endpoint)?;
        if endpoint.scheme() != "https"
            || endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || endpoint.path() != "/"
        {
            return Err(Error::Endpoint);
        }
        Ok(Self {
            endpoint,
            region: input.region,
            bucket: input.bucket,
            prefix: input.prefix,
            access_key_id: Zeroizing::new(input.access_key_id),
            secret_access_key: Zeroizing::new(input.secret_access_key),
            session_token: input.session_token.map(Zeroizing::new),
            server_side_encryption: input.server_side_encryption,
        })
    }
}

#[derive(Debug, Clone)]
pub struct BackupUpload {
    pub backup_id: String,
    pub object_prefix: String,
}

#[derive(Debug, Serialize)]
struct BackupManifest {
    format_version: u8,
    created_at: String,
    files: Vec<BackupFile>,
}

#[derive(Debug, Serialize)]
struct BackupFile {
    name: String,
    bytes: u64,
    sha256: String,
}

struct BackupSourceFile {
    name: &'static str,
    path: PathBuf,
    bytes: u64,
    sha256: String,
}

pub async fn upload_backup(
    destination: &BackupS3Destination,
    backup_directory: &Path,
) -> Result<BackupUpload> {
    let files = tokio::try_join!(
        prepare_file(backup_directory, DATABASE_FILE_NAME),
        prepare_file(backup_directory, SECRETS_FILE_NAME),
    )?;
    let created_at = Utc::now();
    let backup_id = backup_id(created_at);
    let backup_path = format!("backups/{backup_id}");
    let object_prefix = destination.object_key(&backup_path)?;
    let manifest = BackupManifest {
        format_version: 1,
        created_at: created_at.to_rfc3339(),
        files: [&files.0, &files.1]
            .into_iter()
            .map(|file| BackupFile {
                name: file.name.to_owned(),
                bytes: file.bytes,
                sha256: file.sha256.clone(),
            })
            .collect(),
    };
    let client = Client::builder().build().map_err(Error::Http)?;

    for file in [&files.0, &files.1] {
        let key = format!("{backup_path}/{}", file.name);
        put_file(destination, &client, &key, file).await?;
    }
    let manifest = serde_json::to_vec_pretty(&manifest).map_err(Error::Manifest)?;
    let manifest_key = format!("{backup_path}/{MANIFEST_FILE_NAME}");
    put_bytes(
        destination,
        &client,
        &manifest_key,
        manifest,
        "application/json",
    )
    .await?;

    Ok(BackupUpload {
        backup_id,
        object_prefix,
    })
}

async fn prepare_file(directory: &Path, name: &'static str) -> Result<BackupSourceFile> {
    let path = directory.join(name);
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|error| Error::ReadFile(path.clone(), error))?;
    if !metadata.is_file() {
        return Err(Error::ReadFile(
            path,
            std::io::Error::from(std::io::ErrorKind::InvalidInput),
        ));
    }
    Ok(BackupSourceFile {
        name,
        path: path.clone(),
        bytes: metadata.len(),
        sha256: file_sha256(&path).await?,
    })
}

async fn file_sha256(path: &Path) -> Result<String> {
    let mut file = File::open(path)
        .await
        .map_err(|error| Error::ReadFile(path.to_path_buf(), error))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|error| Error::ReadFile(path.to_path_buf(), error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex(hasher.finalize()))
}

async fn put_file(
    destination: &BackupS3Destination,
    client: &Client,
    object_key: &str,
    file: &BackupSourceFile,
) -> Result<()> {
    for attempt in 0..UPLOAD_ATTEMPTS {
        let request = signed_request(destination, client, object_key, &file.sha256, file.bytes)?;
        let source = File::open(&file.path)
            .await
            .map_err(|error| Error::ReadFile(file.path.clone(), error))?;
        let response = request
            .header(CONTENT_TYPE, "application/octet-stream")
            .header(CONTENT_LENGTH, file.bytes)
            .body(reqwest::Body::wrap_stream(ReaderStream::new(source)))
            .send()
            .await;
        if complete_or_retry(response, attempt).await? {
            return Ok(());
        }
    }
    Err(Error::RemoteStatus(
        StatusCode::SERVICE_UNAVAILABLE.as_u16(),
    ))
}

async fn put_bytes(
    destination: &BackupS3Destination,
    client: &Client,
    object_key: &str,
    bytes: Vec<u8>,
    content_type: &'static str,
) -> Result<()> {
    let payload_hash = hex(Sha256::digest(&bytes));
    for attempt in 0..UPLOAD_ATTEMPTS {
        let request = signed_request(
            destination,
            client,
            object_key,
            &payload_hash,
            bytes.len() as u64,
        )?;
        let response = request
            .header(CONTENT_TYPE, content_type)
            .header(CONTENT_LENGTH, bytes.len())
            .body(bytes.clone())
            .send()
            .await;
        if complete_or_retry(response, attempt).await? {
            return Ok(());
        }
    }
    Err(Error::RemoteStatus(
        StatusCode::SERVICE_UNAVAILABLE.as_u16(),
    ))
}

async fn complete_or_retry(
    response: std::result::Result<reqwest::Response, reqwest::Error>,
    attempt: usize,
) -> Result<bool> {
    match response {
        Ok(response) if response.status().is_success() => Ok(true),
        Ok(response) if response.status().is_server_error() && attempt + 1 < UPLOAD_ATTEMPTS => {
            retry_after(attempt).await;
            Ok(false)
        }
        Ok(response) => Err(Error::RemoteStatus(response.status().as_u16())),
        Err(error) if attempt + 1 < UPLOAD_ATTEMPTS => {
            retry_after(attempt).await;
            let _ = error;
            Ok(false)
        }
        Err(error) => Err(Error::Http(error)),
    }
}

async fn retry_after(attempt: usize) {
    sleep(Duration::from_millis(250 * 2_u64.pow(attempt as u32))).await;
}

fn signed_request(
    destination: &BackupS3Destination,
    client: &Client,
    object_key: &str,
    payload_hash: &str,
    content_length: u64,
) -> Result<reqwest::RequestBuilder> {
    let object_url = destination.object_url(object_key)?;
    let now = Utc::now();
    let authorization = authorization(destination, &object_url, payload_hash, now)?;
    let mut request = client
        .put(object_url)
        .header("authorization", authorization)
        .header("x-amz-content-sha256", payload_hash)
        .header("x-amz-date", now.format("%Y%m%dT%H%M%SZ").to_string())
        .header(CONTENT_LENGTH, content_length);
    if let Some(session_token) = &destination.session_token {
        request = request.header("x-amz-security-token", session_token.as_str());
    }
    if destination.server_side_encryption == "AES256" {
        request = request.header("x-amz-server-side-encryption", "AES256");
    }
    Ok(request)
}

fn authorization(
    destination: &BackupS3Destination,
    object_url: &Url,
    payload_hash: &str,
    now: DateTime<Utc>,
) -> Result<String> {
    let host = host_header(object_url)?;
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date = now.format("%Y%m%d").to_string();
    let mut headers = BTreeMap::from([
        ("host", host),
        ("x-amz-content-sha256", payload_hash.to_owned()),
        ("x-amz-date", amz_date.clone()),
    ]);
    if let Some(session_token) = &destination.session_token {
        headers.insert("x-amz-security-token", session_token.to_string());
    }
    if destination.server_side_encryption == "AES256" {
        headers.insert("x-amz-server-side-encryption", "AES256".to_owned());
    }
    let canonical_headers = headers
        .iter()
        .map(|(name, value)| format!("{name}:{value}\n"))
        .collect::<String>();
    let signed_headers = headers.keys().copied().collect::<Vec<_>>().join(";");
    let canonical_request = format!(
        "PUT\n{}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}",
        object_url.path()
    );
    let scope = format!("{date}/{}/s3/aws4_request", destination.region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        hex(Sha256::digest(canonical_request.as_bytes()))
    );
    let date_key = hmac(
        format!("AWS4{}", destination.secret_access_key.as_str()).as_bytes(),
        &date,
    )?;
    let region_key = hmac(&date_key, &destination.region)?;
    let service_key = hmac(&region_key, "s3")?;
    let signing_key = hmac(&service_key, "aws4_request")?;
    let signature = hex(hmac(&signing_key, &string_to_sign)?);
    Ok(format!(
        "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
        destination.access_key_id.as_str()
    ))
}

fn hmac(key: &[u8], message: &str) -> Result<[u8; 32]> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| Error::Signing)?;
    mac.update(message.as_bytes());
    Ok(mac.finalize().into_bytes().into())
}

fn host_header(url: &Url) -> Result<String> {
    let host = url.host_str().ok_or(Error::Endpoint)?;
    Ok(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    })
}

impl BackupS3Destination {
    fn object_key(&self, suffix: &str) -> Result<String> {
        let key = if self.prefix.is_empty() {
            suffix.to_owned()
        } else {
            format!("{}/{suffix}", self.prefix)
        };
        if key.is_empty()
            || key.contains("//")
            || key.split('/').any(|segment| {
                segment.is_empty()
                    || matches!(segment, "." | "..")
                    || !segment.bytes().all(|byte| {
                        byte.is_ascii_lowercase()
                            || byte.is_ascii_uppercase()
                            || byte.is_ascii_digit()
                            || matches!(byte, b'.' | b'_' | b'-')
                    })
            })
        {
            return Err(Error::ObjectKey);
        }
        Ok(key)
    }

    fn object_url(&self, object_key: &str) -> Result<Url> {
        let key = self.object_key(object_key)?;
        Url::parse(&format!(
            "{}/{}/{}",
            self.endpoint.as_str().trim_end_matches('/'),
            self.bucket,
            key
        ))
        .map_err(|_| Error::Endpoint)
    }
}

fn backup_id(now: DateTime<Utc>) -> String {
    format!("{}-{}", now.format("%Y%m%dT%H%M%SZ"), Uuid::new_v4())
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::{BackupS3Destination, BackupS3DestinationInput, authorization};

    #[test]
    fn signs_a_path_style_s3_object_without_credential_in_the_url() {
        let destination = BackupS3Destination::new(BackupS3DestinationInput {
            endpoint: "https://s3.example.test".to_owned(),
            region: "us-east-1".to_owned(),
            bucket: "ignitify-backups".to_owned(),
            prefix: "production".to_owned(),
            access_key_id: "AKIAIOSFODNN7EXAMPLE".to_owned(),
            secret_access_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".to_owned(),
            session_token: Some("session-token".to_owned()),
            server_side_encryption: "AES256".to_owned(),
        })
        .unwrap();
        let url = destination
            .object_url("backups/backup-1/ignitify.db")
            .unwrap();
        let authorization = authorization(
            &destination,
            &url,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            chrono::Utc.with_ymd_and_hms(2026, 8, 9, 8, 0, 0).unwrap(),
        )
        .unwrap();

        assert_eq!(
            url.as_str(),
            "https://s3.example.test/ignitify-backups/production/backups/backup-1/ignitify.db"
        );
        assert!(authorization.starts_with(
            "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20260809/us-east-1/s3/aws4_request"
        ));
        assert!(authorization.contains("SignedHeaders=host;x-amz-content-sha256;x-amz-date;x-amz-security-token;x-amz-server-side-encryption"));
        assert!(authorization.contains("Signature="));
        assert!(!authorization.contains("wJalrXUtnFEMI"));
    }
}
