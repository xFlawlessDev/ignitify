use std::{io::Write, str::FromStr};

use age::{Decryptor, Encryptor, x25519};
use thiserror::Error;
use uuid::Uuid;
use zeroize::Zeroizing;

pub struct AgeCipher {
    identity: x25519::Identity,
    recipient: x25519::Recipient,
}

impl AgeCipher {
    pub fn from_identity(identity: impl AsRef<str>) -> Result<Self> {
        let identity =
            x25519::Identity::from_str(identity.as_ref()).map_err(|_| Error::InvalidIdentity)?;
        let recipient = identity.to_public();
        Ok(Self {
            identity,
            recipient,
        })
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<String> {
        let encryptor =
            Encryptor::with_recipients(std::iter::once(&self.recipient as &dyn age::Recipient))
                .map_err(|_| Error::Encryption)?;
        let mut output = Vec::new();
        {
            let armor =
                age::armor::ArmoredWriter::wrap_output(&mut output, age::armor::Format::AsciiArmor)
                    .map_err(|_| Error::Encryption)?;
            let mut writer = encryptor
                .wrap_output(armor)
                .map_err(|_| Error::Encryption)?;
            writer.write_all(plaintext).map_err(|_| Error::Encryption)?;
            writer
                .finish()
                .map_err(|_| Error::Encryption)?
                .finish()
                .map_err(|_| Error::Encryption)?;
        }
        String::from_utf8(output).map_err(|_| Error::Encryption)
    }

    pub fn decrypt(&self, ciphertext: &str) -> Result<Zeroizing<Vec<u8>>> {
        let decryptor = Decryptor::new(age::armor::ArmoredReader::new(ciphertext.as_bytes()))
            .map_err(|_| Error::InvalidCiphertext)?;
        let mut reader = decryptor
            .decrypt(std::iter::once(&self.identity as &dyn age::Identity))
            .map_err(|_| Error::InvalidCiphertext)?;
        let mut plaintext = Zeroizing::new(Vec::new());
        std::io::Read::read_to_end(&mut reader, &mut plaintext)
            .map_err(|_| Error::InvalidCiphertext)?;
        Ok(plaintext)
    }
}

#[derive(Debug, Clone)]
pub struct ServiceReadModel {
    pub id: String,
    pub project_id: String,
    pub environment_id: String,
    pub role: String,
    pub name: String,
    pub kind: String,
    pub spec: ignitify_domain::ServiceSpec,
    pub source_config: Option<ignitify_domain::ServiceSourceConfig>,
    pub auto_deploy_webhook_secret: Option<Zeroizing<String>>,
    pub deployment_destination_id: Option<String>,
    pub desired_generation: i64,
    pub desired_state: String,
    pub created_at: String,
    pub updated_at: String,
    pub variables: Vec<ServiceVariableReadModel>,
}

#[derive(Debug)]
pub struct AutoDeployWebhookTargetModel {
    pub service_id: String,
    pub provider_id: String,
    pub repository: String,
    pub branch: String,
    pub secret: Zeroizing<String>,
    pub project_owner_id: String,
}

#[derive(Debug)]
pub enum AutoDeploySecretRotation {
    Rotated(Zeroizing<String>),
    Missing,
    Forbidden,
    Disabled,
}

#[derive(Debug, Clone)]
pub struct ProjectEnvironmentVariableInput {
    pub key: String,
    pub value: Option<String>,
    pub is_secret: bool,
}

#[derive(Debug, Clone)]
pub struct ProjectEnvironmentReadModel {
    pub role: String,
    pub variables: Vec<ProjectEnvironmentVariableReadModel>,
}

#[derive(Debug, Clone)]
pub struct ProjectEnvironmentVariableReadModel {
    pub key: String,
    pub is_secret: bool,
    pub is_set: bool,
    pub value: Option<Zeroizing<String>>,
}

#[derive(Debug, Clone)]
pub enum ProjectEnvironmentMutationModel {
    Updated(ProjectEnvironmentReadModel),
    Missing,
    Forbidden,
}

#[derive(Debug, Clone)]
pub struct ServiceVariableReadModel {
    pub key: String,
    pub is_secret: bool,
    pub is_set: bool,
    pub value: Option<Zeroizing<String>>,
}

#[derive(Debug, Clone)]
pub enum ServiceMutationOutcomeModel {
    Created(ServiceReadModel),
    Updated(ServiceReadModel),
    Removed,
    Missing,
    Forbidden,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid age identity")]
    InvalidIdentity,
    #[error("could not encrypt service variable")]
    Encryption,
    #[error("invalid encrypted service variable")]
    InvalidCiphertext,
    #[error("idempotency key must use visible ASCII and be 1 to 128 bytes")]
    InvalidIdempotencyKey,
    #[error("source revision must be a 40 to 64 character lowercase hexadecimal commit id")]
    InvalidSourceRevision,
    #[error("image runtime failed")]
    Runtime,
    #[error("source build failed: {0}")]
    SourceBuild(String),
    #[error("runtime policy rejected input: {0}")]
    Policy(&'static str),
    #[error("worker is unavailable")]
    WorkerUnavailable,
    #[error(transparent)]
    Domain(#[from] ignitify_domain::InputError),
    #[error(transparent)]
    Database(#[from] ignitify_db::DatabaseError),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn validate_idempotency_key(value: &str) -> Result<()> {
    if !(1..=128).contains(&value.len()) || value.bytes().any(|byte| !(0x21..=0x7e).contains(&byte))
    {
        return Err(Error::InvalidIdempotencyKey);
    }
    Ok(())
}

pub(crate) fn validate_source_revision(value: &str) -> Result<()> {
    if !(40..=64).contains(&value.len())
        || value.bytes().any(|byte| !byte.is_ascii_hexdigit())
        || value.bytes().all(|byte| byte == b'0')
    {
        return Err(Error::InvalidSourceRevision);
    }
    Ok(())
}

pub(crate) fn generate_auto_deploy_secret() -> Zeroizing<String> {
    Zeroizing::new(format!(
        "{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    ))
}
