use std::{path::Path, process::Stdio, time::Duration};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use tokio::{fs, process::Command, time::timeout};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::error::ApiError;

use super::{validate_known_hosts, validate_public_key};

pub(super) struct GeneratedKeyPair {
    pub(super) private_key: Zeroizing<Vec<u8>>,
    pub(super) public_key: String,
}

pub(super) async fn generate_key_pair() -> Result<GeneratedKeyPair, ApiError> {
    let directory = std::env::temp_dir().join(format!("ignitify-ssh-keygen-{}", Uuid::new_v4()));
    fs::create_dir(&directory)
        .await
        .map_err(|_| ApiError::RemoteServerSetupFailed)?;
    if let Err(error) = set_directory_permissions(&directory).await {
        let _ = fs::remove_dir_all(&directory).await;
        return Err(error);
    }
    let key_path = directory.join("id_ed25519");
    let key_path_arg = key_path.to_string_lossy().into_owned();
    let result = async {
        let output = timeout(
            Duration::from_secs(10),
            Command::new("ssh-keygen")
                .kill_on_drop(true)
                .args([
                    "-q",
                    "-t",
                    "ed25519",
                    "-N",
                    "",
                    "-C",
                    "ignitify-remote-server",
                    "-f",
                    key_path_arg.as_str(),
                ])
                .env("LANG", "C")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output(),
        )
        .await
        .map_err(|_| ApiError::RemoteServerSetupFailedWithReason("SSH key generation timed out."))?
        .map_err(ssh_keygen_command_error)?;
        if !output.status.success() {
            return Err(ApiError::RemoteServerSetupFailedWithReason(
                "Ignitify could not generate an SSH deployment key.",
            ));
        }
        let private_key = Zeroizing::new(
            fs::read(&key_path)
                .await
                .map_err(|_| ApiError::RemoteServerSetupFailed)?,
        );
        let public_key = fs::read_to_string(key_path.with_extension("pub"))
            .await
            .map_err(|_| ApiError::RemoteServerSetupFailed)?;
        let public_key = validate_public_key(public_key.trim().to_owned())
            .map_err(|_| ApiError::RemoteServerSetupFailed)?;
        Ok(GeneratedKeyPair {
            private_key,
            public_key,
        })
    }
    .await;
    let _ = fs::remove_dir_all(&directory).await;
    result
}

pub(super) async fn scan_known_hosts(host: &str, port: u16) -> Result<String, ApiError> {
    let port = port.to_string();
    let output = timeout(
        Duration::from_secs(15),
        Command::new("ssh-keyscan")
            .kill_on_drop(true)
            .args(["-T", "10", "-p", port.as_str(), host])
            .env("LANG", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output(),
    )
    .await
    .map_err(|_| {
        ApiError::RemoteServerSetupFailedWithReason(
            "SSH host key discovery timed out. Verify the host, port, and firewall rule.",
        )
    })?
    .map_err(ssh_keyscan_command_error)?;
    if !output.status.success() {
        return Err(ApiError::RemoteServerSetupFailedWithReason(
            "Ignitify could not retrieve an SSH host key from this server.",
        ));
    }
    let known_hosts =
        String::from_utf8(output.stdout).map_err(|_| ApiError::RemoteServerSetupFailed)?;
    validate_known_hosts(known_hosts.trim().to_owned()).map_err(|_| {
        ApiError::RemoteServerSetupFailedWithReason(
            "The remote server did not provide a supported SSH host key.",
        )
    })
}

async fn set_directory_permissions(path: &Path) -> Result<(), ApiError> {
    #[cfg(unix)]
    {
        fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .await
            .map_err(|_| ApiError::RemoteServerSetupFailed)?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn ssh_keygen_command_error(error: std::io::Error) -> ApiError {
    if error.kind() == std::io::ErrorKind::NotFound {
        return ApiError::RemoteServerSetupFailedWithReason(
            "SSH keygen utility is not installed on the Ignitify host.",
        );
    }
    ApiError::RemoteServerSetupFailedWithReason("Ignitify could not start SSH key generation.")
}

fn ssh_keyscan_command_error(error: std::io::Error) -> ApiError {
    if error.kind() == std::io::ErrorKind::NotFound {
        return ApiError::RemoteServerSetupFailedWithReason(
            "SSH keyscan utility is not installed on the Ignitify host.",
        );
    }
    ApiError::RemoteServerSetupFailedWithReason("Ignitify could not start SSH host key discovery.")
}
