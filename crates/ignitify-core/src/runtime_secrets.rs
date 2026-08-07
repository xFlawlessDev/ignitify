use std::{
    fs::{self, OpenOptions},
    io::ErrorKind,
    path::Path,
    str::FromStr,
};

use age::{secrecy::ExposeSecret, x25519::Identity};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const DEFAULT_FILE_NAME: &str = "ignitify-secrets.json";

#[derive(Debug, Error)]
pub(super) enum Error {
    #[error("could not create the Ignitify data directory")]
    CreateDirectory(#[source] std::io::Error),
    #[error("could not read the Ignitify runtime secrets")]
    Read(#[source] std::io::Error),
    #[error("the Ignitify runtime secrets file is invalid")]
    InvalidFile(#[source] serde_json::Error),
    #[error("the Ignitify runtime secrets file could not be written")]
    Write(#[source] std::io::Error),
    #[error("the configured age identity is invalid")]
    InvalidIdentity,
    #[error("the configured JWT secret is too short")]
    InvalidJwtSecret,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PersistedSecrets {
    jwt_secret: String,
    secrets_age_identity: String,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeSecrets {
    pub(super) jwt_secret: String,
    pub(super) secrets_age_identity: String,
}

impl RuntimeSecrets {
    pub(super) fn load_or_create(
        data_dir: &Path,
        configured_jwt_secret: Option<&str>,
        configured_age_identity: Option<&str>,
    ) -> Result<Self, Error> {
        fs::create_dir_all(data_dir).map_err(Error::CreateDirectory)?;
        let path = data_dir.join(DEFAULT_FILE_NAME);
        match fs::read_to_string(&path) {
            Ok(contents) => return parse_persisted(&contents),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(Error::Read(error)),
        }

        let secrets = Self {
            jwt_secret: configured_jwt_secret
                .filter(|value| !value.trim().is_empty())
                .map(str::to_owned)
                .unwrap_or_else(generate_jwt_secret),
            secrets_age_identity: configured_age_identity
                .filter(|value| !value.trim().is_empty())
                .map(|value| value.trim().to_owned())
                .unwrap_or_else(|| Identity::generate().to_string().expose_secret().to_owned()),
        };
        validate(&secrets)?;
        let encoded = serde_json::to_vec_pretty(&PersistedSecrets {
            jwt_secret: secrets.jwt_secret.clone(),
            secrets_age_identity: secrets.secrets_age_identity.clone(),
        })
        .map_err(Error::InvalidFile)?;

        match create_secret_file(&path, &encoded) {
            Ok(()) => Ok(secrets),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                let contents = fs::read_to_string(path).map_err(Error::Read)?;
                parse_persisted(&contents)
            }
            Err(error) => Err(Error::Write(error)),
        }
    }
}

fn parse_persisted(contents: &str) -> Result<RuntimeSecrets, Error> {
    let persisted =
        serde_json::from_str::<PersistedSecrets>(contents).map_err(Error::InvalidFile)?;
    let secrets = RuntimeSecrets {
        jwt_secret: persisted.jwt_secret,
        secrets_age_identity: persisted.secrets_age_identity,
    };
    validate(&secrets)?;
    Ok(secrets)
}

fn validate(secrets: &RuntimeSecrets) -> Result<(), Error> {
    if secrets.jwt_secret.trim().len() < 32 {
        return Err(Error::InvalidJwtSecret);
    }
    Identity::from_str(secrets.secrets_age_identity.trim())
        .map(|_| ())
        .map_err(|_| Error::InvalidIdentity)
}

fn generate_jwt_secret() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn create_secret_file(path: &Path, contents: &[u8]) -> Result<(), std::io::Error> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    use std::io::Write;
    file.write_all(contents)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use age::{secrecy::ExposeSecret, x25519::Identity};

    use super::RuntimeSecrets;

    #[test]
    fn creates_and_reuses_persistent_runtime_secrets() {
        let directory = tempfile_dir();
        let identity = Identity::generate().to_string();
        let jwt_secret = "a".repeat(32);
        let first = RuntimeSecrets::load_or_create(
            &directory,
            Some(&jwt_secret),
            Some(identity.expose_secret()),
        )
        .unwrap();
        let second = RuntimeSecrets::load_or_create(&directory, None, None).unwrap();

        assert_eq!(first.jwt_secret, second.jwt_secret);
        assert_eq!(first.secrets_age_identity, second.secrets_age_identity);
        let _ = fs::remove_dir_all(directory);
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("ignitify-secrets-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        directory
    }
}
