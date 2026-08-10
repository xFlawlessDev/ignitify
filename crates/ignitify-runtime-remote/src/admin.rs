use std::{
    collections::HashMap,
    io::{Read, Write},
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::Duration,
};

use ignitify_control_plane::{Error as ControlError, RuntimeContainer, RuntimePort};
use ignitify_runtime_docker::{
    ContainerConfig, ContainerDetails, ContainerMount, ContainerNetwork,
};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde::Deserialize;
use tokio::sync::mpsc as tokio_mpsc;

use super::{MANAGED_LABEL, RemoteSecrets, SshRuntime, shell_quote, terminated_key, write_secret};

mod error;

pub use error::RemoteRuntimeError;
use error::checked_remote_output;

const DEFAULT_TERMINAL_SIZE: PtySize = PtySize {
    rows: 24,
    cols: 80,
    pixel_width: 0,
    pixel_height: 0,
};
const MIN_COLUMNS: u16 = 20;
const MAX_COLUMNS: u16 = 500;
const MIN_ROWS: u16 = 5;
const MAX_ROWS: u16 = 200;
const TERMINAL_COMMAND_WAIT: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy)]
pub struct RemoteRuntimeMetrics {
    pub containers: i64,
    pub containers_running: i64,
    pub images: i64,
    pub cpus: i64,
    pub memory_bytes: i64,
}

impl SshRuntime {
    pub async fn remote_runtime_metrics(
        &self,
        destination_id: &str,
    ) -> Result<RemoteRuntimeMetrics, RemoteRuntimeError> {
        let secrets = self
            .connection(destination_id)
            .await
            .map_err(|_| RemoteRuntimeError::SshUnavailable)?;
        let output = self
            .execute(
                &secrets,
                "set -eu\nif ! command -v docker >/dev/null 2>&1; then\n  printf '%s\\n' IGNITIFY_DOCKER_UNAVAILABLE >&2\n  exit 42\nfi\ndocker info --format '{{json .}}'\n".to_owned(),
            )
            .await
            .map_err(|_| RemoteRuntimeError::SshUnavailable)?;
        let output = checked_remote_output(output)?;
        let info = serde_json::from_str::<DockerInfo>(output.stdout.trim())
            .map_err(|_| RemoteRuntimeError::DockerResponseInvalid)?;
        Ok(RemoteRuntimeMetrics {
            containers: info.containers.unwrap_or_default(),
            containers_running: info.containers_running.unwrap_or_default(),
            images: info.images.unwrap_or_default(),
            cpus: info.cpus.unwrap_or_default(),
            memory_bytes: info.memory_bytes.unwrap_or_default(),
        })
    }

    pub async fn remote_containers(
        &self,
        destination_id: &str,
    ) -> Result<Vec<RuntimeContainer>, RemoteRuntimeError> {
        let secrets = self
            .connection(destination_id)
            .await
            .map_err(|_| RemoteRuntimeError::SshUnavailable)?;
        let output = self
            .execute(
                &secrets,
                "set -eu\nif ! command -v docker >/dev/null 2>&1; then\n  printf '%s\\n' IGNITIFY_DOCKER_UNAVAILABLE >&2\n  exit 42\nfi\ndocker ps --all --quiet | while IFS= read -r id; do\n  docker inspect --format '{{json .}}' \"$id\"\ndone\n".to_owned(),
            )
            .await
            .map_err(|_| RemoteRuntimeError::SshUnavailable)?;
        let output = checked_remote_output(output)?;
        output
            .stdout
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str::<DockerInspect>(line)
                    .map_err(|_| RemoteRuntimeError::DockerResponseInvalid)
                    .and_then(|inspect| {
                        inspect
                            .into_runtime_container()
                            .map_err(|_| RemoteRuntimeError::DockerResponseInvalid)
                    })
            })
            .filter_map(|result| match result {
                Ok(Some(container)) => Some(Ok(container)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }

    pub async fn remote_container_details(
        &self,
        destination_id: &str,
        container_id: &str,
    ) -> Result<ContainerDetails, ControlError> {
        let secrets = self.connection(destination_id).await?;
        self.require_managed_container(&secrets, container_id)
            .await?;
        let inspect = self.inspect_container(&secrets, container_id).await?;
        inspect.into_details()
    }

    pub async fn remote_container_logs(
        &self,
        destination_id: &str,
        container_id: &str,
    ) -> Result<String, ControlError> {
        let secrets = self.connection(destination_id).await?;
        self.require_managed_container(&secrets, container_id)
            .await?;
        let output = self
            .execute(
                &secrets,
                format!(
                    "set -eu\ndocker logs --timestamps --tail 200 {} 2>&1\n",
                    shell_quote(container_id)
                ),
            )
            .await?;
        if output.success {
            Ok(output.stdout)
        } else {
            Err(ControlError::Runtime)
        }
    }

    pub async fn remove_remote_container(
        &self,
        destination_id: &str,
        container_id: &str,
    ) -> Result<(), ControlError> {
        let secrets = self.connection(destination_id).await?;
        self.require_managed_container(&secrets, container_id)
            .await?;
        let output = self
            .execute(
                &secrets,
                format!("set -eu\ndocker rm --force {}\n", shell_quote(container_id)),
            )
            .await?;
        output.success.then_some(()).ok_or(ControlError::Runtime)
    }

    pub async fn upload_remote_container_file(
        &self,
        destination_id: &str,
        container_id: &str,
        destination: &str,
        file_name: &str,
        data: &[u8],
    ) -> Result<(), ControlError> {
        validate_upload_target(destination, file_name)?;
        let secrets = self.connection(destination_id).await?;
        self.require_managed_container(&secrets, container_id)
            .await?;
        let destination = format!(
            "{}:{}/{}",
            container_id,
            destination.trim_end_matches('/'),
            file_name
        );
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data);
        let output = self
            .execute(
                &secrets,
                format!(
                    "set -eu\ntemporary=$(mktemp)\ntrap 'rm -f \"$temporary\"' EXIT\nprintf '%s' {} | base64 -d > \"$temporary\"\ndocker cp \"$temporary\" {}\n",
                    shell_quote(&encoded),
                    shell_quote(&destination)
                ),
            )
            .await?;
        output.success.then_some(()).ok_or(ControlError::Runtime)
    }

    pub async fn open_remote_host_terminal(
        &self,
        destination_id: &str,
    ) -> Result<RemoteTerminalSession, ControlError> {
        let secrets = self.connection(destination_id).await?;
        self.open_remote_terminal(secrets, None).await
    }

    pub async fn open_remote_container_terminal(
        &self,
        destination_id: &str,
        container_id: &str,
    ) -> Result<RemoteTerminalSession, ControlError> {
        let secrets = self.connection(destination_id).await?;
        self.require_managed_container(&secrets, container_id)
            .await?;
        self.open_remote_terminal(secrets, Some(container_id.to_owned()))
            .await
    }

    async fn require_managed_container(
        &self,
        secrets: &RemoteSecrets,
        container_id: &str,
    ) -> Result<(), ControlError> {
        validate_container_id(container_id)?;
        let output = self
            .execute(
                secrets,
                format!(
                    "set -eu\ndocker inspect --format '{{{{json .Config.Labels}}}}' {}\n",
                    shell_quote(container_id)
                ),
            )
            .await?;
        if !output.success {
            return Err(ControlError::Runtime);
        }
        let labels = serde_json::from_str::<HashMap<String, String>>(output.stdout.trim())
            .map_err(|_| ControlError::Runtime)?;
        (labels
            .get(MANAGED_LABEL)
            .is_some_and(|value| value == "true"))
        .then_some(())
        .ok_or(ControlError::Runtime)
    }

    async fn inspect_container(
        &self,
        secrets: &RemoteSecrets,
        container_id: &str,
    ) -> Result<DockerInspect, ControlError> {
        let output = self
            .execute(
                secrets,
                format!(
                    "set -eu\ndocker inspect --format '{{{{json .}}}}' {}\n",
                    shell_quote(container_id)
                ),
            )
            .await?;
        if !output.success {
            return Err(ControlError::Runtime);
        }
        serde_json::from_str(output.stdout.trim()).map_err(|_| ControlError::Runtime)
    }

    async fn open_remote_terminal(
        &self,
        secrets: RemoteSecrets,
        container_id: Option<String>,
    ) -> Result<RemoteTerminalSession, ControlError> {
        let directory = super::tempfile_directory();
        let result = async {
            tokio::fs::create_dir(&directory)
                .await
                .map_err(|_| ControlError::Runtime)?;
            set_terminal_directory_permissions(&directory).await?;
            let key_path = directory.join("id_key");
            let known_hosts_path = directory.join("known_hosts");
            write_secret(&key_path, &terminated_key(&secrets.private_key)).await?;
            write_secret(&known_hosts_path, &secrets.known_hosts).await?;
            let (command_sender, command_receiver) = mpsc::channel();
            let (event_sender, event_receiver) = tokio_mpsc::channel(128);
            let config = RemoteTerminalConfig {
                key_path: key_path.to_string_lossy().into_owned(),
                known_hosts_path: known_hosts_path.to_string_lossy().into_owned(),
                port: secrets.connection.port.to_string(),
                target: format!(
                    "{}@{}",
                    secrets.connection.username, secrets.connection.host
                ),
                container_id,
            };
            let worker_directory = directory.clone();
            thread::Builder::new()
                .name("ignitify-remote-terminal".to_owned())
                .spawn(move || {
                    run_remote_terminal(config, worker_directory, command_receiver, event_sender)
                })
                .map_err(|_| ControlError::Runtime)?;
            Ok(RemoteTerminalSession {
                command_sender,
                event_receiver,
            })
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_dir_all(&directory).await;
        }
        result
    }
}

#[derive(Debug, Deserialize)]
struct DockerInfo {
    #[serde(rename = "Containers")]
    containers: Option<i64>,
    #[serde(rename = "ContainersRunning")]
    containers_running: Option<i64>,
    #[serde(rename = "Images")]
    images: Option<i64>,
    #[serde(rename = "NCPU")]
    cpus: Option<i64>,
    #[serde(rename = "MemTotal")]
    memory_bytes: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct DockerInspect {
    #[serde(rename = "Id")]
    id: Option<String>,
    #[serde(rename = "Name")]
    name: Option<String>,
    #[serde(rename = "RestartCount")]
    restart_count: Option<i64>,
    #[serde(rename = "Config")]
    config: Option<DockerConfig>,
    #[serde(rename = "State")]
    state: Option<DockerState>,
    #[serde(rename = "HostConfig")]
    host_config: Option<DockerHostConfig>,
    #[serde(rename = "NetworkSettings")]
    network_settings: Option<DockerNetworkSettings>,
    #[serde(rename = "Mounts", default)]
    mounts: Vec<DockerMount>,
}

impl DockerInspect {
    fn into_runtime_container(self) -> Result<Option<RuntimeContainer>, ControlError> {
        let config = self.config.as_ref().ok_or(ControlError::Runtime)?;
        if !config
            .labels
            .as_ref()
            .and_then(|labels| labels.get(MANAGED_LABEL))
            .is_some_and(|value| value == "true")
        {
            return Ok(None);
        }
        let state = self.state.as_ref();
        let state_name = state
            .and_then(|value| value.status.as_deref())
            .unwrap_or("unknown")
            .to_owned();
        let status = if state.is_some_and(|value| value.running.unwrap_or(false)) {
            "Up".to_owned()
        } else {
            state_name.clone()
        };
        Ok(Some(RuntimeContainer {
            id: self.id.unwrap_or_default(),
            name: self
                .name
                .unwrap_or_default()
                .trim_start_matches('/')
                .to_owned(),
            image: config.image.clone().unwrap_or_default(),
            state: state_name,
            status,
            health: state
                .and_then(|value| value.health.as_ref())
                .and_then(|health| health.status.clone()),
            ports: self
                .network_settings
                .as_ref()
                .map(DockerNetworkSettings::ports)
                .unwrap_or_default(),
            restart_count: self.restart_count.unwrap_or_default(),
            cpu_percentage: None,
            memory_usage_bytes: None,
            cpu_limit_nano_cpus: self
                .host_config
                .as_ref()
                .and_then(|config| config.nano_cpus),
            memory_limit_bytes: self.host_config.as_ref().and_then(|config| config.memory),
            managed: true,
        }))
    }

    fn into_details(self) -> Result<ContainerDetails, ControlError> {
        let config = self.config.ok_or(ControlError::Runtime)?;
        let state = self.state.unwrap_or_default();
        let status = state.status.unwrap_or_else(|| "unknown".to_owned());
        let restart_policy = self
            .host_config
            .as_ref()
            .and_then(|host| host.restart_policy.as_ref())
            .and_then(|policy| policy.name.clone());
        let privileged = self
            .host_config
            .as_ref()
            .and_then(|host| host.privileged)
            .unwrap_or(false);
        Ok(ContainerDetails {
            id: self.id.unwrap_or_default(),
            name: self
                .name
                .unwrap_or_default()
                .trim_start_matches('/')
                .to_owned(),
            image: config.image.unwrap_or_default(),
            state: status.clone(),
            status,
            config: ContainerConfig {
                command: config.command.unwrap_or_default(),
                entrypoint: config.entrypoint.unwrap_or_default(),
                user: config.user,
                working_dir: config.working_dir,
                tty: config.tty.unwrap_or(false),
                environment_keys: config
                    .environment
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|entry| entry.split_once('=').map(|(key, _)| key.to_owned()))
                    .collect(),
                labels: config.labels.unwrap_or_default().into_iter().collect(),
                restart_policy,
                privileged,
            },
            mounts: self
                .mounts
                .into_iter()
                .map(|mount| ContainerMount {
                    kind: mount.kind.unwrap_or_else(|| "unknown".to_owned()),
                    source: mount.source,
                    destination: mount.destination,
                    read_only: mount.read_only.unwrap_or(false),
                })
                .collect(),
            networks: self
                .network_settings
                .map(DockerNetworkSettings::networks)
                .unwrap_or_default(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct DockerConfig {
    #[serde(rename = "Image")]
    image: Option<String>,
    #[serde(rename = "Cmd")]
    command: Option<Vec<String>>,
    #[serde(rename = "Entrypoint")]
    entrypoint: Option<Vec<String>>,
    #[serde(rename = "User")]
    user: Option<String>,
    #[serde(rename = "WorkingDir")]
    working_dir: Option<String>,
    #[serde(rename = "Tty")]
    tty: Option<bool>,
    #[serde(rename = "Env")]
    environment: Option<Vec<String>>,
    #[serde(rename = "Labels")]
    labels: Option<HashMap<String, String>>,
}

#[derive(Debug, Default, Deserialize)]
struct DockerState {
    #[serde(rename = "Status")]
    status: Option<String>,
    #[serde(rename = "Running")]
    running: Option<bool>,
    #[serde(rename = "Health")]
    health: Option<DockerHealth>,
}

#[derive(Debug, Deserialize)]
struct DockerHealth {
    #[serde(rename = "Status")]
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DockerHostConfig {
    #[serde(rename = "NanoCpus")]
    nano_cpus: Option<i64>,
    #[serde(rename = "Memory")]
    memory: Option<i64>,
    #[serde(rename = "Privileged")]
    privileged: Option<bool>,
    #[serde(rename = "RestartPolicy")]
    restart_policy: Option<DockerRestartPolicy>,
}

#[derive(Debug, Deserialize)]
struct DockerRestartPolicy {
    #[serde(rename = "Name")]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DockerNetworkSettings {
    #[serde(rename = "Ports")]
    ports: Option<HashMap<String, Option<Vec<DockerPortBinding>>>>,
    #[serde(rename = "Networks")]
    networks: Option<HashMap<String, DockerNetwork>>,
}

impl DockerNetworkSettings {
    fn ports(&self) -> Vec<RuntimePort> {
        self.ports
            .as_ref()
            .into_iter()
            .flat_map(|ports| ports.iter())
            .flat_map(|(container, bindings)| {
                let (port, protocol) = container.split_once('/').unwrap_or((container, "tcp"));
                let container_port = port.parse::<u16>().ok();
                bindings.as_ref().into_iter().flatten().map(move |binding| {
                    container_port.map(|container_port| RuntimePort {
                        container_port,
                        host_ip: binding.host_ip.clone(),
                        host_port: binding
                            .host_port
                            .as_deref()
                            .and_then(|port| port.parse::<u16>().ok()),
                        protocol: protocol.to_owned(),
                    })
                })
            })
            .flatten()
            .collect()
    }

    fn networks(self) -> Vec<ContainerNetwork> {
        self.networks
            .unwrap_or_default()
            .into_iter()
            .map(|(name, network)| ContainerNetwork {
                name,
                ip_address: network.ip_address,
                gateway: network.gateway,
                mac_address: network.mac_address,
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct DockerPortBinding {
    #[serde(rename = "HostIp")]
    host_ip: Option<String>,
    #[serde(rename = "HostPort")]
    host_port: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DockerNetwork {
    #[serde(rename = "IPAddress")]
    ip_address: Option<String>,
    #[serde(rename = "Gateway")]
    gateway: Option<String>,
    #[serde(rename = "MacAddress")]
    mac_address: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DockerMount {
    #[serde(rename = "Type")]
    kind: Option<String>,
    #[serde(rename = "Source")]
    source: Option<String>,
    #[serde(rename = "Destination")]
    destination: Option<String>,
    #[serde(rename = "RW")]
    read_only: Option<bool>,
}

fn validate_container_id(value: &str) -> Result<(), ControlError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(ControlError::Runtime);
    }
    Ok(())
}

fn validate_upload_target(destination: &str, file_name: &str) -> Result<(), ControlError> {
    if !destination.starts_with('/')
        || destination.len() > 1024
        || destination.split('/').any(|part| part == "..")
        || file_name.is_empty()
        || file_name.len() > 255
        || file_name.contains(['/', '\\', '\0'])
    {
        return Err(ControlError::Runtime);
    }
    Ok(())
}

pub enum RemoteTerminalEvent {
    Output(Vec<u8>),
    Exited,
    Unavailable,
}

pub struct RemoteTerminalSession {
    command_sender: mpsc::Sender<RemoteTerminalCommand>,
    event_receiver: tokio_mpsc::Receiver<RemoteTerminalEvent>,
}

impl RemoteTerminalSession {
    pub fn input(&self, input: Vec<u8>) -> Result<(), ControlError> {
        self.command_sender
            .send(RemoteTerminalCommand::Input(input))
            .map_err(|_| ControlError::Runtime)
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), ControlError> {
        self.command_sender
            .send(RemoteTerminalCommand::Resize(PtySize {
                cols: cols.clamp(MIN_COLUMNS, MAX_COLUMNS),
                rows: rows.clamp(MIN_ROWS, MAX_ROWS),
                pixel_width: 0,
                pixel_height: 0,
            }))
            .map_err(|_| ControlError::Runtime)
    }

    pub async fn next_event(&mut self) -> Option<RemoteTerminalEvent> {
        self.event_receiver.recv().await
    }

    pub fn close(&self) {
        let _ = self.command_sender.send(RemoteTerminalCommand::Close);
    }
}

impl Drop for RemoteTerminalSession {
    fn drop(&mut self) {
        self.close();
    }
}

enum RemoteTerminalCommand {
    Input(Vec<u8>),
    Resize(PtySize),
    Close,
}

struct RemoteTerminalConfig {
    key_path: String,
    known_hosts_path: String,
    port: String,
    target: String,
    container_id: Option<String>,
}

fn run_remote_terminal(
    config: RemoteTerminalConfig,
    directory: std::path::PathBuf,
    commands: mpsc::Receiver<RemoteTerminalCommand>,
    events: tokio_mpsc::Sender<RemoteTerminalEvent>,
) {
    let pty_system = native_pty_system();
    let pair = match pty_system.openpty(DEFAULT_TERMINAL_SIZE) {
        Ok(pair) => pair,
        Err(_) => {
            emit_terminal(&events, RemoteTerminalEvent::Unavailable);
            let _ = std::fs::remove_dir_all(directory);
            return;
        }
    };
    let mut command = CommandBuilder::new("ssh");
    let user_known_hosts = format!("UserKnownHostsFile={}", config.known_hosts_path);
    let global_known_hosts = format!(
        "GlobalKnownHostsFile={}",
        if cfg!(windows) { "NUL" } else { "/dev/null" }
    );
    command.args([
        "-F",
        "none",
        "-i",
        &config.key_path,
        "-o",
        "IdentitiesOnly=yes",
        "-o",
        "PasswordAuthentication=no",
        "-o",
        "BatchMode=yes",
        "-o",
        "StrictHostKeyChecking=yes",
        "-o",
        &user_known_hosts,
        "-o",
        &global_known_hosts,
        "-o",
        "ConnectTimeout=10",
        "-p",
        &config.port,
        "-tt",
        &config.target,
    ]);
    if let Some(container_id) = config.container_id.as_deref() {
        command.args(["docker", "exec", "-it", container_id, "/bin/sh"]);
    }
    command.env("LANG", "C");
    let mut child = match pair.slave.spawn_command(command) {
        Ok(child) => child,
        Err(_) => {
            emit_terminal(&events, RemoteTerminalEvent::Unavailable);
            let _ = std::fs::remove_dir_all(directory);
            return;
        }
    };
    drop(pair.slave);
    let mut reader = match pair.master.try_clone_reader() {
        Ok(reader) => reader,
        Err(_) => {
            emit_terminal(&events, RemoteTerminalEvent::Unavailable);
            let _ = child.kill();
            let _ = std::fs::remove_dir_all(directory);
            return;
        }
    };
    let reader_events = events.clone();
    let _ = thread::Builder::new()
        .name("ignitify-remote-terminal-output".to_owned())
        .spawn(move || read_terminal_output(&mut reader, reader_events));
    let mut writer = match pair.master.take_writer() {
        Ok(writer) => writer,
        Err(_) => {
            emit_terminal(&events, RemoteTerminalEvent::Unavailable);
            let _ = child.kill();
            let _ = std::fs::remove_dir_all(directory);
            return;
        }
    };
    loop {
        match commands.recv_timeout(TERMINAL_COMMAND_WAIT) {
            Ok(RemoteTerminalCommand::Input(input)) => {
                if writer
                    .write_all(&input)
                    .and_then(|_| writer.flush())
                    .is_err()
                {
                    break;
                }
            }
            Ok(RemoteTerminalCommand::Resize(size)) => {
                if pair.master.resize(size).is_err() {
                    break;
                }
            }
            Ok(RemoteTerminalCommand::Close) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        match child.try_wait() {
            Ok(Some(_)) => {
                emit_terminal(&events, RemoteTerminalEvent::Exited);
                let _ = std::fs::remove_dir_all(directory);
                return;
            }
            Ok(None) => {}
            Err(_) => break,
        }
    }
    let _ = child.kill();
    emit_terminal(&events, RemoteTerminalEvent::Exited);
    let _ = std::fs::remove_dir_all(directory);
}

fn emit_terminal(events: &tokio_mpsc::Sender<RemoteTerminalEvent>, event: RemoteTerminalEvent) {
    let _ = events.blocking_send(event);
}

fn read_terminal_output(
    reader: &mut (dyn Read + Send),
    events: tokio_mpsc::Sender<RemoteTerminalEvent>,
) {
    let mut buffer = [0_u8; 8192];
    loop {
        let count = match reader.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(count) => count,
        };
        emit_terminal(
            &events,
            RemoteTerminalEvent::Output(buffer[..count].to_vec()),
        );
    }
}

async fn set_terminal_directory_permissions(path: &std::path::Path) -> Result<(), ControlError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .await
            .map_err(|_| ControlError::Runtime)?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests;
