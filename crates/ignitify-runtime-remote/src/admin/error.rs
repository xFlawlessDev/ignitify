use super::super::RemoteOutput;

#[derive(Debug, Clone, Copy)]
pub enum RemoteRuntimeError {
    SshUnavailable,
    DockerUnavailable,
    DockerAccessDenied,
    DockerDaemonUnavailable,
    DockerCommandFailed,
    DockerResponseInvalid,
}

impl RemoteRuntimeError {
    pub const fn user_message(self) -> &'static str {
        match self {
            Self::SshUnavailable => {
                "The Ignitify host could not establish the configured SSH connection."
            }
            Self::DockerUnavailable => {
                "Docker is not installed or is not available to the configured SSH user."
            }
            Self::DockerAccessDenied => {
                "The configured SSH user is not allowed to access the Docker daemon."
            }
            Self::DockerDaemonUnavailable => {
                "The Docker daemon is not running on the remote server."
            }
            Self::DockerCommandFailed => {
                "A remote Docker command failed. Check Docker and the remote host configuration."
            }
            Self::DockerResponseInvalid => {
                "The remote Docker installation returned an unsupported response."
            }
        }
    }
}

pub(super) fn checked_remote_output(
    output: RemoteOutput,
) -> Result<RemoteOutput, RemoteRuntimeError> {
    if output.success {
        return Ok(output);
    }

    let stderr = output.stderr.to_ascii_lowercase();
    let error = if stderr.contains("permission denied (publickey)")
        || stderr.contains("host key verification failed")
        || stderr.contains("could not resolve hostname")
        || stderr.contains("connection refused")
        || stderr.contains("connection timed out")
        || stderr.contains("network is unreachable")
    {
        RemoteRuntimeError::SshUnavailable
    } else if stderr.contains("ignitify_docker_unavailable")
        || stderr.contains("docker: not found")
        || stderr.contains("docker: command not found")
        || stderr.contains("command -v docker")
    {
        RemoteRuntimeError::DockerUnavailable
    } else if stderr
        .contains("permission denied while trying to connect to the docker daemon socket")
        || stderr.contains("permission denied") && stderr.contains("docker.sock")
    {
        RemoteRuntimeError::DockerAccessDenied
    } else if stderr.contains("cannot connect to the docker daemon")
        || stderr.contains("is the docker daemon running")
    {
        RemoteRuntimeError::DockerDaemonUnavailable
    } else {
        RemoteRuntimeError::DockerCommandFailed
    };
    Err(error)
}
