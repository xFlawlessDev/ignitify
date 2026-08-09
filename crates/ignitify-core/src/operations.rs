use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

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
) -> Result<(), Error> {
    match command {
        Command::Backup { output } => backup(&output, data_dir, database_config).await,
        Command::Restore { source } => restore(&source, data_dir, database_config),
    }
}

async fn backup(
    output: &Path,
    data_dir: &Path,
    database_config: &ignitify_db::DatabaseConfig,
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
    let snapshot = database.backup_to(&snapshot_path).await;
    database.close().await;
    if let Err(error) = snapshot {
        let _ = fs::remove_dir_all(output);
        return Err(error.into());
    }
    if let Err(error) = copy_sensitive_file(&secret_path, &output.join(runtime_secrets::FILE_NAME))
    {
        let _ = fs::remove_dir_all(output);
        return Err(error);
    }

    println!("Ignitify backup created at {}", output.display());
    Ok(())
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
    let header = fs::read(path).map_err(|error| match error.kind() {
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
    let contents = fs::read(source).map_err(|error| match error.kind() {
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
        if let Err(error) = fs::rename(target, recovery) {
            restore_recovery(&moved);
            return Err(error.into());
        }
        moved.push((target.clone(), recovery.clone()));
    }
    Ok(moved)
}

fn install_replacements(replacements: &[(PathBuf, PathBuf)]) -> Result<(), Error> {
    for (staged, target) in replacements {
        fs::rename(staged, target)?;
    }
    Ok(())
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
    #[error(transparent)]
    RuntimeSecrets(#[from] runtime_secrets::Error),
    #[error(transparent)]
    Database(#[from] ignitify_db::DatabaseError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{backup, restore};
    use crate::runtime_secrets::{self, RuntimeSecrets};

    #[tokio::test]
    async fn backup_and_restore_preserve_database_and_runtime_secrets() {
        let root =
            std::env::temp_dir().join(format!("ignitify-operations-{}", uuid::Uuid::new_v4()));
        let data_dir = root.join("data");
        let database_path = root.join("ignitify.db");
        let backup_dir = root.join("backup");
        fs::create_dir_all(&data_dir).unwrap();
        RuntimeSecrets::load_or_create(&data_dir, Some(&"x".repeat(32)), None).unwrap();
        let config = ignitify_db::DatabaseConfig {
            url: format!("sqlite:{}", database_path.display()),
        };
        let database = ignitify_db::Database::connect(&config).await.unwrap();
        database.ping().await.unwrap();
        database.close().await;

        backup(&backup_dir, &data_dir, &config).await.unwrap();
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
