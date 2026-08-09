use std::pin::Pin;

use bollard::{
    container::{LogsOptions, RemoveContainerOptions, UploadToContainerOptions},
    exec::{CreateExecOptions, ResizeExecOptions, StartExecOptions, StartExecResults},
    models::ContainerInspectResponse,
};
use futures_util::StreamExt;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use super::{DockerRuntime, Error, MANAGED_LABEL, Result, is_not_found};

const MAX_LOG_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerDetails {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub config: ContainerConfig,
    pub mounts: Vec<ContainerMount>,
    pub networks: Vec<ContainerNetwork>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerConfig {
    pub command: Vec<String>,
    pub entrypoint: Vec<String>,
    pub user: Option<String>,
    pub working_dir: Option<String>,
    pub tty: bool,
    pub environment_keys: Vec<String>,
    pub labels: Vec<(String, String)>,
    pub restart_policy: Option<String>,
    pub privileged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerMount {
    pub kind: String,
    pub source: Option<String>,
    pub destination: Option<String>,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerNetwork {
    pub name: String,
    pub ip_address: Option<String>,
    pub gateway: Option<String>,
    pub mac_address: Option<String>,
}

pub enum ContainerTerminalEvent {
    Output(Vec<u8>),
    Exited,
}

pub struct ContainerTerminalSession {
    docker: bollard::Docker,
    exec_id: String,
    output: Pin<
        Box<
            dyn futures_util::Stream<
                    Item = std::result::Result<
                        bollard::container::LogOutput,
                        bollard::errors::Error,
                    >,
                > + Send,
        >,
    >,
    input: Pin<Box<dyn AsyncWrite + Send>>,
}

impl DockerRuntime {
    pub async fn container_details(&self, container_id: &str) -> Result<ContainerDetails> {
        validate_container_reference(container_id)?;
        let inspected = self.inspect_managed_container(container_id).await?;
        Ok(details_from_inspect(inspected, container_id))
    }

    pub async fn container_logs(&self, container_id: &str) -> Result<String> {
        validate_container_reference(container_id)?;
        self.inspect_managed_container(container_id).await?;
        let mut output = Vec::new();
        let mut logs = self.docker.logs(
            container_id,
            Some(LogsOptions {
                follow: false,
                stdout: true,
                stderr: true,
                timestamps: true,
                tail: "200".to_owned(),
                ..Default::default()
            }),
        );
        while let Some(item) = logs.next().await {
            let item = item?;
            let bytes = item.as_ref();
            let remaining = MAX_LOG_BYTES.saturating_sub(output.len());
            if remaining == 0 {
                break;
            }
            output.extend_from_slice(&bytes[..bytes.len().min(remaining)]);
        }
        Ok(String::from_utf8_lossy(&output).into_owned())
    }

    pub async fn remove_container(&self, container_id: &str) -> Result<()> {
        validate_container_reference(container_id)?;
        self.inspect_managed_container(container_id).await?;
        match self
            .docker
            .remove_container(
                container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
        {
            Ok(()) => Ok(()),
            Err(error) if is_not_found(&error) => Err(Error::ContainerNotFound),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn upload_file(
        &self,
        container_id: &str,
        destination: &str,
        file_name: &str,
        data: &[u8],
    ) -> Result<()> {
        validate_container_reference(container_id)?;
        self.inspect_managed_container(container_id).await?;
        validate_upload_path(destination)?;
        validate_file_name(file_name)?;
        let archive = single_file_tar(file_name, data);
        self.docker
            .upload_to_container(
                container_id,
                Some(UploadToContainerOptions {
                    path: destination.to_owned(),
                    no_overwrite_dir_non_dir: "0".to_owned(),
                }),
                archive.into(),
            )
            .await
            .map_err(Into::into)
    }

    pub async fn open_terminal(&self, container_id: &str) -> Result<ContainerTerminalSession> {
        validate_container_reference(container_id)?;
        let inspected = self.inspect_managed_container(container_id).await?;
        let running = inspected
            .state
            .as_ref()
            .and_then(|state| state.running)
            .unwrap_or(false);
        if !running {
            return Err(Error::ContainerNotRunning);
        }
        let created = self
            .docker
            .create_exec(
                container_id,
                CreateExecOptions {
                    attach_stdin: Some(true),
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    tty: Some(true),
                    cmd: Some(vec!["/bin/sh".to_owned()]),
                    ..Default::default()
                },
            )
            .await?;
        let started = self
            .docker
            .start_exec(
                &created.id,
                Some(StartExecOptions {
                    detach: false,
                    tty: true,
                    ..Default::default()
                }),
            )
            .await?;
        let StartExecResults::Attached { output, input } = started else {
            return Err(Error::TerminalUnavailable);
        };
        Ok(ContainerTerminalSession {
            docker: self.docker.clone(),
            exec_id: created.id,
            output,
            input,
        })
    }

    async fn inspect_container(&self, container_id: &str) -> Result<ContainerInspectResponse> {
        self.docker
            .inspect_container(container_id, None)
            .await
            .map_err(|error| {
                if is_not_found(&error) {
                    Error::ContainerNotFound
                } else {
                    error.into()
                }
            })
    }

    async fn inspect_managed_container(
        &self,
        container_id: &str,
    ) -> Result<ContainerInspectResponse> {
        let inspected = self.inspect_container(container_id).await?;
        let managed = inspected
            .config
            .as_ref()
            .and_then(|config| config.labels.as_ref())
            .and_then(|labels| labels.get(MANAGED_LABEL))
            .is_some_and(|value| value == "true");
        if managed {
            Ok(inspected)
        } else {
            Err(Error::ContainerNotManaged)
        }
    }
}

impl ContainerTerminalSession {
    pub async fn next_event(&mut self) -> Result<Option<ContainerTerminalEvent>> {
        match self.output.next().await {
            None => Ok(Some(ContainerTerminalEvent::Exited)),
            Some(Ok(output)) => Ok(Some(ContainerTerminalEvent::Output(
                output.as_ref().to_vec(),
            ))),
            Some(Err(error)) => Err(error.into()),
        }
    }

    pub async fn input(&mut self, data: &[u8]) -> Result<()> {
        self.input.write_all(data).await.map_err(Into::into)
    }

    pub async fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.docker
            .resize_exec(
                &self.exec_id,
                ResizeExecOptions {
                    width: cols,
                    height: rows,
                },
            )
            .await
            .map_err(Into::into)
    }
}

fn details_from_inspect(
    inspected: ContainerInspectResponse,
    fallback_id: &str,
) -> ContainerDetails {
    let config = inspected.config.unwrap_or_default();
    let state = inspected.state.unwrap_or_default();
    let state_name = state
        .status
        .map(|status| status.to_string())
        .unwrap_or_else(|| "unknown".to_owned());
    let labels = config.labels.clone().unwrap_or_default();
    let mut labels = labels.into_iter().collect::<Vec<_>>();
    labels.sort_by(|left, right| left.0.cmp(&right.0));
    let environment_keys = config
        .env
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| entry.split_once('=').map(|(key, _)| key.to_owned()))
        .collect();
    let restart_policy = inspected
        .host_config
        .as_ref()
        .and_then(|host| host.restart_policy.as_ref())
        .and_then(|policy| policy.name.map(|name| name.to_string()));
    let privileged = inspected
        .host_config
        .as_ref()
        .and_then(|host| host.privileged)
        .unwrap_or(false);
    let mounts = inspected
        .mounts
        .unwrap_or_default()
        .into_iter()
        .map(|mount| ContainerMount {
            kind: mount.typ.map(|kind| kind.to_string()).unwrap_or_default(),
            source: mount.source,
            destination: mount.destination,
            read_only: !mount.rw.unwrap_or(true),
        })
        .collect();
    let networks = inspected
        .network_settings
        .and_then(|settings| settings.networks)
        .unwrap_or_default()
        .into_iter()
        .map(|(name, network)| ContainerNetwork {
            name,
            ip_address: network.ip_address,
            gateway: network.gateway,
            mac_address: network.mac_address,
        })
        .collect();
    ContainerDetails {
        id: inspected.id.unwrap_or_else(|| fallback_id.to_owned()),
        name: inspected
            .name
            .unwrap_or_else(|| fallback_id.to_owned())
            .trim_start_matches('/')
            .to_owned(),
        image: config.image.unwrap_or_default(),
        state: state_name.clone(),
        status: state_name,
        config: ContainerConfig {
            command: config.cmd.unwrap_or_default(),
            entrypoint: config.entrypoint.unwrap_or_default(),
            user: config.user.filter(|value| !value.is_empty()),
            working_dir: config.working_dir.filter(|value| !value.is_empty()),
            tty: config.tty.unwrap_or(false),
            environment_keys,
            labels,
            restart_policy,
            privileged,
        },
        mounts,
        networks,
    }
}

fn validate_container_reference(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(Error::InvalidContainerReference);
    }
    Ok(())
}

fn validate_upload_path(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 1024
        || !value.starts_with('/')
        || value.contains('\0')
        || value.split('/').any(|part| part == "..")
    {
        return Err(Error::InvalidUploadPath);
    }
    Ok(())
}

fn validate_file_name(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 100
        || value == "."
        || value == ".."
        || value.contains(['/', '\\', '\0'])
    {
        return Err(Error::InvalidUploadPath);
    }
    Ok(())
}

fn single_file_tar(name: &str, data: &[u8]) -> Vec<u8> {
    let mut header = [0_u8; 512];
    copy_field(&mut header[0..100], name.as_bytes());
    write_octal(&mut header[100..108], 0o644);
    write_octal(&mut header[108..116], 0);
    write_octal(&mut header[116..124], 0);
    write_octal(&mut header[124..136], data.len() as u64);
    write_octal(&mut header[136..148], 0);
    header[148..156].fill(b' ');
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    let checksum = header.iter().map(|byte| u64::from(*byte)).sum();
    write_octal(&mut header[148..156], checksum);

    let padding = (512 - data.len() % 512) % 512;
    let mut archive = Vec::with_capacity(512 + data.len() + padding + 1024);
    archive.extend_from_slice(&header);
    archive.extend_from_slice(data);
    archive.resize(archive.len() + padding + 1024, 0);
    archive
}

fn copy_field(field: &mut [u8], value: &[u8]) {
    let length = value.len().min(field.len());
    field[..length].copy_from_slice(&value[..length]);
}

fn write_octal(field: &mut [u8], value: u64) {
    let digits = format!("{value:o}");
    let start = field.len().saturating_sub(digits.len() + 1);
    field.fill(b'0');
    field[start..start + digits.len()].copy_from_slice(digits.as_bytes());
    field[field.len() - 1] = 0;
}

#[cfg(test)]
mod tests {
    use super::{single_file_tar, validate_container_reference, validate_file_name};

    #[test]
    fn tar_archive_contains_a_single_regular_file() {
        let archive = single_file_tar("hello.txt", b"hello");
        assert_eq!(&archive[0..9], b"hello.txt");
        assert_eq!(archive[156], b'0');
        assert_eq!(&archive[512..517], b"hello");
        assert_eq!(archive.len() % 512, 0);
    }

    #[test]
    fn container_reference_rejects_path_separators() {
        assert!(validate_container_reference("container_name-1").is_ok());
        assert!(validate_container_reference("../container").is_err());
    }

    #[test]
    fn upload_file_name_rejects_path_traversal() {
        assert!(validate_file_name("report.txt").is_ok());
        assert!(validate_file_name("../report.txt").is_err());
    }
}
