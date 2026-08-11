use std::{path::Path as FsPath, process::Stdio, time::Duration};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
};
use chrono::Utc;
use ignitify_auth::AuthError;
use ignitify_control_plane::AgeCipher;
use ignitify_db::{RemoteServerAgentHeartbeat, RemoteServerAgentRecord};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tokio::{fs, io::AsyncWriteExt, process::Command, time::timeout};
use url::Url;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    error::ApiError,
    extract::{require_actor, require_same_origin_request},
    state::AppState,
};

const AGENT_VERSION: &str = "0.1.0";
const AGENT_HEARTBEAT_INTERVAL_SECONDS: u64 = 30;
const MAX_AGENT_VERSION_BYTES: usize = 64;

#[derive(Debug, Serialize)]
pub(crate) struct RemoteServerAgentResponse {
    pub(crate) status: String,
    pub(crate) version: Option<String>,
    pub(crate) cpu_usage_percentage: Option<f64>,
    pub(crate) cpu_cores: Option<i64>,
    pub(crate) memory_used_bytes: Option<i64>,
    pub(crate) memory_total_bytes: Option<i64>,
    pub(crate) disk_used_bytes: Option<i64>,
    pub(crate) disk_total_bytes: Option<i64>,
    pub(crate) docker_containers: Option<i64>,
    pub(crate) docker_running_containers: Option<i64>,
    pub(crate) last_heartbeat_at: Option<String>,
    pub(crate) last_error: Option<String>,
    pub(crate) installed_at: String,
    pub(crate) updated_at: String,
}

impl From<RemoteServerAgentRecord> for RemoteServerAgentResponse {
    fn from(value: RemoteServerAgentRecord) -> Self {
        Self {
            status: value.status,
            version: value.version,
            cpu_usage_percentage: value.cpu_usage_percentage,
            cpu_cores: value.cpu_cores,
            memory_used_bytes: value.memory_used_bytes,
            memory_total_bytes: value.memory_total_bytes,
            disk_used_bytes: value.disk_used_bytes,
            disk_total_bytes: value.disk_total_bytes,
            docker_containers: value.docker_containers,
            docker_running_containers: value.docker_running_containers,
            last_heartbeat_at: value.last_heartbeat_at,
            last_error: value.last_error,
            installed_at: value.installed_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct RemoteServerAgentInstallResponse {
    pub(crate) agent: RemoteServerAgentResponse,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoteServerAgentHeartbeatRequest {
    server_id: String,
    version: String,
    #[serde(default)]
    cpu_usage_percentage: Option<f64>,
    #[serde(default)]
    cpu_cores: Option<i64>,
    #[serde(default)]
    memory_used_bytes: Option<i64>,
    #[serde(default)]
    memory_total_bytes: Option<i64>,
    #[serde(default)]
    disk_used_bytes: Option<i64>,
    #[serde(default)]
    disk_total_bytes: Option<i64>,
    #[serde(default)]
    docker_containers: Option<i64>,
    #[serde(default)]
    docker_running_containers: Option<i64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RemoteServerAgentHeartbeatResponse {
    accepted: bool,
}

pub(crate) async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(server_id): Path<String>,
) -> Result<Json<Option<RemoteServerAgentResponse>>, ApiError> {
    require_admin(&state, &headers).await?;
    if state
        .database
        .remote_servers()
        .connection(&server_id)
        .await?
        .is_none()
    {
        return Err(ApiError::NotFound);
    }
    let agent = state
        .database
        .remote_server_agents()
        .get(&server_id)
        .await?
        .map(Into::into);
    Ok(Json(agent))
}

pub(crate) async fn install(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(server_id): Path<String>,
) -> Result<(StatusCode, Json<RemoteServerAgentInstallResponse>), ApiError> {
    require_admin(&state, &headers).await?;
    require_same_origin_request(&state, &headers)?;
    let endpoint = agent_endpoint(&state)?;
    let connection = state
        .database
        .remote_servers()
        .connection(&server_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let cipher = provider_cipher(&state)?;
    let private_key = cipher
        .decrypt(&connection.private_key_ciphertext)
        .map_err(|_| ApiError::RemoteAgentProvisionFailed)?;
    let known_hosts = cipher
        .decrypt(&connection.known_hosts_ciphertext)
        .map_err(|_| ApiError::RemoteAgentProvisionFailed)?;
    let token = Uuid::new_v4().to_string();
    let token_hash = hash_token(&token);
    state
        .database
        .remote_server_agents()
        .install(&server_id, &token_hash)
        .await?;
    let script = provisioning_script(&connection.id, &endpoint, &token);
    if let Err(error) = run_ssh_script(
        &connection,
        private_key.as_ref(),
        known_hosts.as_ref(),
        &script,
    )
    .await
    {
        let _ = state
            .database
            .remote_server_agents()
            .record_error(&server_id, "remote agent provisioning failed")
            .await;
        return Err(error);
    }
    let agent = state
        .database
        .remote_server_agents()
        .get(&server_id)
        .await?
        .map(Into::into)
        .ok_or(ApiError::NotFound)?;
    Ok((
        StatusCode::OK,
        Json(RemoteServerAgentInstallResponse { agent }),
    ))
}

pub(crate) async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RemoteServerAgentHeartbeatRequest>,
) -> Result<Json<RemoteServerAgentHeartbeatResponse>, ApiError> {
    let token = bearer_token(&headers).ok_or(AuthError::InvalidToken)?;
    let server_id = Uuid::parse_str(&request.server_id)
        .map_err(|_| ApiError::BadRequest("agent server_id is invalid"))?
        .to_string();
    validate_heartbeat(&request)?;
    let token_hash = state
        .database
        .remote_server_agents()
        .token_hash(&server_id)
        .await?
        .ok_or(AuthError::InvalidToken)?;
    if !bool::from(hash_token(token).as_bytes().ct_eq(token_hash.as_bytes())) {
        return Err(AuthError::InvalidToken.into());
    }
    let heartbeat = RemoteServerAgentHeartbeat {
        version: request.version,
        cpu_usage_percentage: request.cpu_usage_percentage,
        cpu_cores: request.cpu_cores,
        memory_used_bytes: request.memory_used_bytes,
        memory_total_bytes: request.memory_total_bytes,
        disk_used_bytes: request.disk_used_bytes,
        disk_total_bytes: request.disk_total_bytes,
        docker_containers: request.docker_containers,
        docker_running_containers: request.docker_running_containers,
        reported_at: Utc::now().to_rfc3339(),
    };
    state
        .database
        .remote_server_agents()
        .record_heartbeat(&server_id, &heartbeat)
        .await?
        .ok_or(AuthError::InvalidToken)?;
    Ok(Json(RemoteServerAgentHeartbeatResponse { accepted: true }))
}

fn validate_heartbeat(request: &RemoteServerAgentHeartbeatRequest) -> Result<(), ApiError> {
    if request.version.is_empty() || request.version.len() > MAX_AGENT_VERSION_BYTES {
        return Err(ApiError::BadRequest("agent version is invalid"));
    }
    let metrics = [
        request.cpu_cores,
        request.memory_used_bytes,
        request.memory_total_bytes,
        request.disk_used_bytes,
        request.disk_total_bytes,
        request.docker_containers,
        request.docker_running_containers,
    ];
    if metrics.into_iter().flatten().any(|value| value < 0) {
        return Err(ApiError::BadRequest("agent metrics are invalid"));
    }
    if request
        .cpu_usage_percentage
        .is_some_and(|value| !value.is_finite() || !(0.0..=100.0).contains(&value))
    {
        return Err(ApiError::BadRequest("agent CPU usage is invalid"));
    }
    Ok(())
}

fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .filter(|value| !value.is_empty())
}

fn agent_endpoint(state: &AppState) -> Result<String, ApiError> {
    state
        .origin_policy
        .public_origin()
        .and_then(|origin| {
            let url = Url::parse(&origin).ok()?;
            if !matches!(url.scheme(), "http" | "https") {
                return None;
            }
            let host = url.host_str()?;
            if matches!(host, "localhost" | "127.0.0.1" | "::1") {
                return None;
            }
            Some(format!(
                "{}/api/v1/remote-agents/heartbeat",
                origin.trim_end_matches('/')
            ))
        })
        .ok_or(ApiError::RemoteAgentEndpointUnavailable)
}

fn provider_cipher(state: &AppState) -> Result<&std::sync::Arc<AgeCipher>, ApiError> {
    state
        .provider_cipher
        .as_ref()
        .ok_or(ApiError::ProviderCapabilityUnavailable)
}

async fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    if require_actor(state, headers)
        .await?
        .has_platform_operator_access()
    {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

async fn run_ssh_script(
    connection: &ignitify_db::RemoteServerConnection,
    private_key: &[u8],
    known_hosts: &[u8],
    script: &str,
) -> Result<(), ApiError> {
    let directory = std::env::temp_dir().join(format!("ignitify-agent-{}", Uuid::new_v4()));
    fs::create_dir(&directory)
        .await
        .map_err(|_| ApiError::RemoteAgentProvisionFailed)?;
    set_directory_permissions(&directory).await?;
    let private_key_path = directory.join("id_key");
    let known_hosts_path = directory.join("known_hosts");
    let result = async {
        write_private_key_file(&private_key_path, private_key).await?;
        write_secret_file(&known_hosts_path, known_hosts).await?;
        let private_key_arg = private_key_path.to_string_lossy().into_owned();
        let known_hosts_option = format!("UserKnownHostsFile={}", known_hosts_path.display());
        let global_known_hosts_option = format!(
            "GlobalKnownHostsFile={}",
            if cfg!(windows) { "NUL" } else { "/dev/null" }
        );
        let port = connection.port.to_string();
        let target = format!("{}@{}", connection.username, connection.host);
        let mut child = Command::new("ssh")
            .kill_on_drop(true)
            .args([
                "-F",
                "none",
                "-i",
                private_key_arg.as_str(),
                "-o",
                "IdentitiesOnly=yes",
                "-o",
                "PasswordAuthentication=no",
                "-o",
                "BatchMode=yes",
                "-o",
                "StrictHostKeyChecking=yes",
                "-o",
                known_hosts_option.as_str(),
                "-o",
                global_known_hosts_option.as_str(),
                "-o",
                "ConnectTimeout=10",
                "-p",
                port.as_str(),
                target.as_str(),
                "sh",
                "-s",
            ])
            .env("LANG", "C")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| ApiError::RemoteAgentProvisionFailed)?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(script.as_bytes())
                .await
                .map_err(|_| ApiError::RemoteAgentProvisionFailed)?;
        }
        let output = timeout(Duration::from_secs(45), child.wait_with_output())
            .await
            .map_err(|_| ApiError::RemoteAgentProvisionFailed)?
            .map_err(|_| ApiError::RemoteAgentProvisionFailed)?;
        if output.status.success() {
            Ok(())
        } else {
            Err(ApiError::RemoteAgentProvisionFailed)
        }
    }
    .await;
    let _ = fs::remove_dir_all(&directory).await;
    result
}

async fn write_private_key_file(path: &FsPath, contents: &[u8]) -> Result<(), ApiError> {
    let mut terminated = Zeroizing::new(contents.to_vec());
    if !terminated.ends_with(b"\n") {
        terminated.push(b'\n');
    }
    write_secret_file(path, terminated.as_ref()).await
}

async fn write_secret_file(path: &FsPath, contents: &[u8]) -> Result<(), ApiError> {
    fs::write(path, contents)
        .await
        .map_err(|_| ApiError::RemoteAgentProvisionFailed)?;
    #[cfg(unix)]
    fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .await
        .map_err(|_| ApiError::RemoteAgentProvisionFailed)?;
    Ok(())
}

async fn set_directory_permissions(path: &FsPath) -> Result<(), ApiError> {
    #[cfg(unix)]
    fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .await
        .map_err(|_| ApiError::RemoteAgentProvisionFailed)?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn provisioning_script(server_id: &str, endpoint: &str, token: &str) -> String {
    r#"set -eu
if [ "$(id -u)" -eq 0 ]; then SUDO=""; else command -v sudo >/dev/null 2>&1 || exit 20; SUDO="sudo -n"; fi
command -v curl >/dev/null 2>&1 || exit 21
command -v systemctl >/dev/null 2>&1 || exit 22
$SUDO install -d -m 700 /etc/ignitify-agent
$SUDO sh -c 'cat > /etc/ignitify-agent/agent.env' <<'EOF'
IGNITIFY_AGENT_ENDPOINT=__ENDPOINT__
IGNITIFY_AGENT_TOKEN=__TOKEN__
IGNITIFY_AGENT_SERVER_ID=__SERVER_ID__
EOF
$SUDO chmod 600 /etc/ignitify-agent/agent.env
$SUDO sh -c 'cat > /usr/local/lib/ignitify-agent.sh' <<'SCRIPT'
#!/bin/sh
set -eu
. /etc/ignitify-agent/agent.env
while :; do
  memory_total=$(awk '/MemTotal:/ {print $2 * 1024; exit}' /proc/meminfo)
  memory_available=$(awk '/MemAvailable:/ {print $2 * 1024; exit}' /proc/meminfo)
  memory_used=$((memory_total - memory_available))
  disk_total=$(df -Pk / | awk 'NR == 2 {print $2 * 1024}')
  disk_used=$(df -Pk / | awk 'NR == 2 {print $3 * 1024}')
  cpu_cores=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 1)
  docker_containers=0
  docker_running_containers=0
  if command -v docker >/dev/null 2>&1; then
    docker_containers=$(docker ps -a -q 2>/dev/null | wc -l | tr -d ' ')
    docker_running_containers=$(docker ps -q 2>/dev/null | wc -l | tr -d ' ')
  fi
  payload=$(printf '{"server_id":"%s","version":"__AGENT_VERSION__","cpu_cores":%s,"memory_used_bytes":%s,"memory_total_bytes":%s,"disk_used_bytes":%s,"disk_total_bytes":%s,"docker_containers":%s,"docker_running_containers":%s}' "$IGNITIFY_AGENT_SERVER_ID" "$cpu_cores" "$memory_used" "$memory_total" "$disk_used" "$disk_total" "$docker_containers" "$docker_running_containers")
  curl -fsS --max-time 10 -X POST "$IGNITIFY_AGENT_ENDPOINT" -H "Authorization: Bearer $IGNITIFY_AGENT_TOKEN" -H 'Content-Type: application/json' --data "$payload" >/dev/null || true
  sleep __HEARTBEAT_INTERVAL_SECONDS__
done
SCRIPT
$SUDO chmod 700 /usr/local/lib/ignitify-agent.sh
$SUDO sh -c 'cat > /etc/systemd/system/ignitify-agent.service' <<'UNIT'
[Unit]
Description=Ignitify remote host monitoring agent
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/lib/ignitify-agent.sh
Restart=always
RestartSec=5
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
UNIT
$SUDO systemctl daemon-reload
$SUDO systemctl enable --now ignitify-agent.service
"#
    .replace("__ENDPOINT__", &shell_quote(endpoint))
    .replace("__TOKEN__", &shell_quote(token))
    .replace("__SERVER_ID__", &shell_quote(server_id))
    .replace("__AGENT_VERSION__", AGENT_VERSION)
    .replace(
        "__HEARTBEAT_INTERVAL_SECONDS__",
        &AGENT_HEARTBEAT_INTERVAL_SECONDS.to_string(),
    )
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
