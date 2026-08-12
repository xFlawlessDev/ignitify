use std::collections::BTreeMap;

use zeroize::Zeroizing;

use crate::{AgeCipher, Error, Result};

pub(crate) fn decrypt_deployment_environment(
    cipher: &AgeCipher,
    ciphertext: &str,
) -> Result<Vec<String>> {
    Ok(decrypt_deployment_values(cipher, ciphertext)?
        .into_iter()
        .map(|(key, value)| format!("{key}={}", value.as_str()))
        .collect())
}

pub(crate) fn deployment_secret_values(
    cipher: &AgeCipher,
    ciphertext: &str,
) -> Result<Vec<Zeroizing<String>>> {
    Ok(decrypt_deployment_values(cipher, ciphertext)?
        .into_values()
        .filter(|value| !value.is_empty())
        .collect())
}

pub(crate) fn redact_logs(
    logs: Vec<ignitify_db::NewDeploymentLog>,
    values: &[Zeroizing<String>],
) -> Vec<ignitify_db::NewDeploymentLog> {
    if values.is_empty() {
        return logs;
    }
    logs.into_iter()
        .map(|mut log| {
            log.line = "[REDACTED]".to_owned();
            log
        })
        .collect()
}

pub(crate) fn decrypt_deployment_values(
    cipher: &AgeCipher,
    ciphertext: &str,
) -> Result<BTreeMap<String, Zeroizing<String>>> {
    let plaintext = cipher.decrypt(ciphertext)?;
    serde_json::from_slice(plaintext.as_slice()).map_err(|_| Error::InvalidCiphertext)
}
