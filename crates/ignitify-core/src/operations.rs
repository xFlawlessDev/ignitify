use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use ignitify_backup_s3::{BackupS3Destination, BackupS3DestinationInput, upload_backup};
use ignitify_control_plane::AgeCipher;
use thiserror::Error;

use crate::runtime_secrets::{self, RuntimeSecrets};

const DATABASE_FILE_NAME: &str = "ignitify.db";

#[derive(Debug)]
pub(super) enum Command {
    Backup { output: PathBuf },
    Restore { source: PathBuf },
}

impl Command {
    pub(super) fn from_environment() -> Result<Option<Self>, Error> {
        let mut arguments = env::args_os();
        let _binary = arguments.next();
        let Some(operation) = arguments.next() else {
            return Ok(None);
        };
        let operation = operation.to_string_lossy();
        let command = match operation.as_ref() {
            "backup" => Self::Backup {
                output: PathBuf::from(required_argument(
                    &mut arguments,
                    "backup output directory",
                )?),
            },
            "restore" => {
                let source = PathBuf::from(required_argument(&mut arguments, "backup directory")?);
                let confirmation = arguments.next();
                if confirmation.as_deref() != Some("--confirm-offline".as_ref()) {
                    return Err(Error::RestoreConfirmationRequired);
                }
                Self::Restore { source }
            }
            _ => return Err(Error::UnknownCommand(operation.into_owned())),
        };
        if arguments.next().is_some() {
            return Err(Error::UnexpectedArgument);
        }
        Ok(Some(command))
    }
}

pub(super) async fn execute(
    command: Command,
    data_dir: &Path,
    database_config: &ignitify_db::DatabaseConfig,
    secrets_age_identity: &str,
) -> Result<(), Error> {
    match command {
        Command::Backup { output } => {
            backup(&output, data_dir, database_config, secrets_age_identity).await
        }
        Command::Restore { source } => restore(&source, data_dir, database_config),
    }
}

async fn backup(
    output: &Path,
    data_dir: &Path,
    database_config: &ignitify_db::DatabaseConfig,
    secrets_age_identity: &str,
) -> Result<(), Error> {
    if output.exists() {
        return Err(Error::OutputExists(output.to_path_buf()));
    }
    let database_path = database_config.file_path().ok_or(Error::MemoryDatabase)?;
    if !database_path.is_file() {
        return Err(Error::MissingFile(database_path));
    }
    let secret_path = runtime_secrets::secret_file_path(data_dir);
    RuntimeSecrets::validate_backup_file(&secret_path)?;

    fs::create_dir_all(output)?;
    let snapshot_path = output.join(DATABASE_FILE_NAME);
    let database = ignitify_db::Database::connect(database_config).await?;
    let s3_destination = database.backup_destinations().s3_connection().await?;
    let snapshot = database.backup_to(&snapshot_path).await;
    database.close().await;
    if let Err(error) = snapshot {
        let _ = fs::remove_dir_all(output);
        return Err(error.into());
    }
    if let Err(error) = remove_s3_destination_from_snapshot(&snapshot_path).await {
        let _ = fs::remove_dir_all(output);
        return Err(error);
    }
    if let Err(error) = copy_sensitive_file(&secret_path, &output.join(runtime_secrets::FILE_NAME))
    {
        let _ = fs::remove_dir_all(output);
        return Err(error);
    }

    println!("Ignitify backup created locally at {}", output.display());
    if let Some(connection) = s3_destination {
        let destination = decrypt_s3_destination(&connection, secrets_age_identity)?;
        let upload = upload_backup(&destination, output).await?;
        println!(
            "Ignitify backup uploaded to s3://{}/{}",
            connection.bucket, upload.object_prefix
        );
    }
    Ok(())
}

async fn remove_s3_destination_from_snapshot(snapshot_path: &Path) -> Result<(), Error> {
    let suffix = unique_suffix()?.to_string();
    let redacted = staged_path(snapshot_path, &format!("s3-redacted-{suffix}"));
    let previous = staged_path(snapshot_path, &format!("s3-original-{suffix}"));
    let snapshot_config = ignitify_db::DatabaseConfig {
        url: format!("sqlite:{}", snapshot_path.display()),
    };
    let snapshot = ignitify_db::Database::connect(&snapshot_config).await?;
    let redact = snapshot.backup_destinations().delete_s3().await;
    let compact = if redact.is_ok() {
        snapshot.backup_to(&redacted).await
    } else {
        Ok(())
    };
    snapshot.close().await;
    if let Err(error) = redact {
        let _ = fs::remove_file(&redacted);
        return Err(error.into());
    }
    if let Err(error) = compact {
        let _ = fs::remove_file(&redacted);
        return Err(error.into());
    }

    rename_with_retry(snapshot_path, &previous)?;
    if let Err(error) = rename_with_retry(&redacted, snapshot_path) {
        let _ = rename_with_retry(&previous, snapshot_path);
        let _ = fs::remove_file(&redacted);
        return Err(error.into());
    }
    let _ = fs::remove_file(&previous);
    let _ = fs::remove_file(sqlite_sidecar(snapshot_path, "-wal"));
    let _ = fs::remove_file(sqlite_sidecar(snapshot_path, "-shm"));
    Ok(())
}

fn decrypt_s3_destination(
    connection: &ignitify_db::BackupS3DestinationConnection,
    secrets_age_identity: &str,
) -> Result<BackupS3Destination, Error> {
    let cipher = AgeCipher::from_identity(secrets_age_identity)?;
    let access_key_id = decrypt_s3_credential(&cipher, &connection.access_key_id_ciphertext)?;
    let secret_access_key =
        decrypt_s3_credential(&cipher, &connection.secret_access_key_ciphertext)?;
    let session_token = connection
        .session_token_ciphertext
        .as_deref()
        .map(|ciphertext| decrypt_s3_credential(&cipher, ciphertext))
        .transpose()?;
    Ok(BackupS3Destination::new(BackupS3DestinationInput {
        endpoint: connection.endpoint.clone(),
        region: connection.region.clone(),
        bucket: connection.bucket.clone(),
        prefix: connection.prefix.clone(),
        access_key_id,
        secret_access_key,
        session_token,
        server_side_encryption: connection.server_side_encryption.clone(),
    })?)
}

fn decrypt_s3_credential(cipher: &AgeCipher, ciphertext: &str) -> Result<String, Error> {
    let decrypted = cipher.decrypt(ciphertext)?;
    let value = std::str::from_utf8(&decrypted).map_err(|_| Error::InvalidS3Credential)?;
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(Error::InvalidS3Credential);
    }
    Ok(value.to_owned())
}

fn restore(
    source: &Path,
    data_dir: &Path,
    database_config: &ignitify_db::DatabaseConfig,
) -> Result<(), Error> {
    let source_database = source.join(DATABASE_FILE_NAME);
    let source_secrets = source.join(runtime_secrets::FILE_NAME);
    validate_database_snapshot(&source_database)?;
    RuntimeSecrets::validate_backup_file(&source_secrets)?;

    let target_database = database_config.file_path().ok_or(Error::MemoryDatabase)?;
    let target_secrets = runtime_secrets::secret_file_path(data_dir);
    if source_database == target_database || source_secrets == target_secrets {
        return Err(Error::RestoreSourceMatchesTarget);
    }
    let database_parent = target_database.parent().ok_or(Error::InvalidDatabasePath)?;
    fs::create_dir_all(database_parent)?;
    fs::create_dir_all(data_dir)?;

    let nonce = unique_suffix()?.to_string();
    let staged_database = staged_path(&target_database, &nonce);
    let staged_secrets = staged_path(&target_secrets, &nonce);
    copy_sensitive_file(&source_database, &staged_database)?;
    if let Err(error) = copy_sensitive_file(&source_secrets, &staged_secrets) {
        let _ = fs::remove_file(&staged_database);
        return Err(error);
    }

    let recovery = data_dir.join(format!("restore-recovery-{nonce}"));
    fs::create_dir(&recovery)?;
    let targets = vec![
        (target_database.clone(), recovery.join(DATABASE_FILE_NAME)),
        (
            sqlite_sidecar(&target_database, "-wal"),
            recovery.join(format!("{DATABASE_FILE_NAME}-wal")),
        ),
        (
            sqlite_sidecar(&target_database, "-shm"),
            recovery.join(format!("{DATABASE_FILE_NAME}-shm")),
        ),
        (
            target_secrets.clone(),
            recovery.join(runtime_secrets::FILE_NAME),
        ),
    ];
    let moved = move_existing_to_recovery(&targets)?;
    let replacements = [
        (staged_database.clone(), target_database.clone()),
        (staged_secrets.clone(), target_secrets.clone()),
    ];
    if let Err(error) = install_replacements(&replacements) {
        let _ = fs::remove_file(&target_database);
        let _ = fs::remove_file(&target_secrets);
        restore_recovery(&moved);
        let _ = fs::remove_file(&staged_database);
        let _ = fs::remove_file(&staged_secrets);
        return Err(error);
    }

    println!(
        "Ignitify restore completed. Previous files are preserved at {}",
        recovery.display()
    );
    Ok(())
}

fn required_argument(
    arguments: &mut impl Iterator<Item = OsString>,
    name: &str,
) -> Result<OsString, Error> {
    arguments
        .next()
        .ok_or_else(|| Error::MissingArgument(name.to_owned()))
}

fn validate_database_snapshot(path: &Path) -> Result<(), Error> {
    let header = read_with_retry(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => Error::MissingFile(path.to_path_buf()),
        _ => Error::Io(error),
    })?;
    if header.starts_with(b"SQLite format 3\0") {
        Ok(())
    } else {
        Err(Error::InvalidDatabaseSnapshot(path.to_path_buf()))
    }
}

fn copy_sensitive_file(source: &Path, destination: &Path) -> Result<(), Error> {
    let contents = read_with_retry(source).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => Error::MissingFile(source.to_path_buf()),
        _ => Error::Io(error),
    })?;
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(destination)?;
    use std::io::Write;
    file.write_all(&contents)?;
    file.sync_all()?;
    Ok(())
}

fn staged_path(target: &Path, nonce: &str) -> PathBuf {
    let mut value = target.as_os_str().to_owned();
    value.push(format!(".restore-{nonce}.tmp"));
    PathBuf::from(value)
}

fn sqlite_sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_owned();
    value.push(suffix);
    PathBuf::from(value)
}

fn move_existing_to_recovery(
    targets: &[(PathBuf, PathBuf)],
) -> Result<Vec<(PathBuf, PathBuf)>, Error> {
    let mut moved = Vec::new();
    for (target, recovery) in targets {
        if !target.exists() {
            continue;
        }
        if let Err(error) = rename_with_retry(target, recovery) {
            restore_recovery(&moved);
            return Err(error.into());
        }
        moved.push((target.clone(), recovery.clone()));
    }
    Ok(moved)
}

fn install_replacements(replacements: &[(PathBuf, PathBuf)]) -> Result<(), Error> {
    for (staged, target) in replacements {
        rename_with_retry(staged, target)?;
    }
    Ok(())
}

fn read_with_retry(path: &Path) -> std::io::Result<Vec<u8>> {
    retry_sharing_violation(|| fs::read(path))
}

fn rename_with_retry(source: &Path, destination: &Path) -> std::io::Result<()> {
    retry_sharing_violation(|| fs::rename(source, destination))
}

fn retry_sharing_violation<T>(
    mut operation: impl FnMut() -> std::io::Result<T>,
) -> std::io::Result<T> {
    let mut result = operation();
    for _ in 0..9 {
        if !matches!(&result, Err(error) if error.raw_os_error() == Some(32)) {
            return result;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
        result = operation();
    }
    result
}

fn restore_recovery(moved: &[(PathBuf, PathBuf)]) {
    for (target, recovery) in moved.iter().rev() {
        if !target.exists() && recovery.exists() {
            let _ = fs::rename(recovery, target);
        }
    }
}

fn unique_suffix() -> Result<u128, Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .map_err(|_| Error::Clock)
}

#[derive(Debug, Error)]
pub(super) enum Error {
    #[error("unknown operation: {0}")]
    UnknownCommand(String),
    #[error("missing {0}")]
    MissingArgument(String),
    #[error("unexpected additional argument")]
    UnexpectedArgument,
    #[error("restore requires the --confirm-offline acknowledgement")]
    RestoreConfirmationRequired,
    #[error("backup output already exists: {0}")]
    OutputExists(PathBuf),
    #[error("SQLite in-memory databases cannot be backed up or restored by this command")]
    MemoryDatabase,
    #[error("database path has no parent directory")]
    InvalidDatabasePath,
    #[error("required backup file is missing: {0}")]
    MissingFile(PathBuf),
    #[error("backup database is not a valid SQLite snapshot: {0}")]
    InvalidDatabaseSnapshot(PathBuf),
    #[error("backup source must not be the live data directory")]
    RestoreSourceMatchesTarget,
    #[error("system clock is before the Unix epoch")]
    Clock,
    #[error("S3 backup credentials are invalid")]
    InvalidS3Credential,
    #[error(transparent)]
    RuntimeSecrets(#[from] runtime_secrets::Error),
    #[error(transparent)]
    Database(#[from] ignitify_db::DatabaseError),
    #[error(transparent)]
    Control(#[from] ignitify_control_plane::Error),
    #[error(transparent)]
    S3(#[from] ignitify_backup_s3::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use std::fs;

    use age::secrecy::ExposeSecret;
    use ignitify_control_plane::AgeCipher;

    use super::{backup, remove_s3_destination_from_snapshot, restore};
    use crate::runtime_secrets::{self, RuntimeSecrets};

    #[tokio::test]
    async fn backup_and_restore_preserve_database_and_runtime_secrets() {
        let root =
            std::env::temp_dir().join(format!("ignitify-operations-{}", uuid::Uuid::new_v4()));
        let data_dir = root.join("data");
        let database_path = root.join("ignitify.db");
        let backup_dir = root.join("backup");
        fs::create_dir_all(&data_dir).unwrap();
        let secrets =
            RuntimeSecrets::load_or_create(&data_dir, Some(&"x".repeat(32)), None).unwrap();
        let config = ignitify_db::DatabaseConfig {
            url: format!("sqlite:{}", database_path.display()),
        };
        let database = ignitify_db::Database::connect(&config).await.unwrap();
        database.ping().await.unwrap();
        database.close().await;

        backup(
            &backup_dir,
            &data_dir,
            &config,
            &secrets.secrets_age_identity,
        )
        .await
        .unwrap();
        let snapshot = fs::read(backup_dir.join("ignitify.db")).unwrap();
        write_file_with_retry(&database_path, b"SQLite format 3\0modified");
        write_file_with_retry(runtime_secrets::secret_file_path(&data_dir), b"invalid");

        restore(&backup_dir, &data_dir, &config).unwrap();

        assert_eq!(fs::read(&database_path).unwrap(), snapshot);
        RuntimeSecrets::validate_backup_file(&runtime_secrets::secret_file_path(&data_dir))
            .unwrap();
        assert!(
            fs::read_dir(&data_dir)
                .unwrap()
                .filter_map(|entry| entry.ok())
                .any(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("restore-recovery-"))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn backup_snapshot_excludes_s3_upload_credentials() {
        let root =
            std::env::temp_dir().join(format!("ignitify-operations-{}", uuid::Uuid::new_v4()));
        let database_path = root.join("ignitify.db");
        let snapshot_path = root.join("snapshot.db");
        fs::create_dir_all(&root).unwrap();
        let config = ignitify_db::DatabaseConfig {
            url: format!("sqlite:{}", database_path.display()),
        };
        let database = ignitify_db::Database::connect(&config).await.unwrap();
        let identity = age::x25519::Identity::generate().to_string();
        let cipher = AgeCipher::from_identity(identity.expose_secret()).unwrap();
        database
            .backup_destinations()
            .upsert_s3(ignitify_db::NewBackupS3Destination {
                endpoint: "https://s3.example.test".to_owned(),
                region: "us-east-1".to_owned(),
                bucket: "ignitify-backups".to_owned(),
                prefix: "production".to_owned(),
                access_key_id_ciphertext: cipher.encrypt(b"access-key").unwrap(),
                secret_access_key_ciphertext: cipher.encrypt(b"secret-key").unwrap(),
                session_token_ciphertext: None,
                server_side_encryption: "AES256".to_owned(),
            })
            .await
            .unwrap();
        database.backup_to(&snapshot_path).await.unwrap();
        database.close().await;

        remove_s3_destination_from_snapshot(&snapshot_path)
            .await
            .unwrap();

        let snapshot = ignitify_db::Database::connect(&ignitify_db::DatabaseConfig {
            url: format!("sqlite:{}", snapshot_path.display()),
        })
        .await
        .unwrap();
        assert!(
            snapshot
                .backup_destinations()
                .s3_connection()
                .await
                .unwrap()
                .is_none()
        );
        snapshot.close().await;
        let _ = fs::remove_dir_all(root);
    }

    fn write_file_with_retry(path: impl AsRef<std::path::Path>, contents: &[u8]) {
        let path = path.as_ref();
        for attempt in 0..10 {
            match fs::write(path, contents) {
                Ok(()) => return,
                Err(error) if error.raw_os_error() == Some(32) && attempt < 9 => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => panic!("could not write test fixture: {error}"),
            }
        }
    }
}
