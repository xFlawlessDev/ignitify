use std::{path::Path, process::Stdio, sync::Arc, time::Duration};

mod admin;

pub use admin::{RemoteRuntimeMetrics, RemoteTerminalEvent, RemoteTerminalSession};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use ignitify_control_plane::{
    AgeCipher, Error as ControlError, ImageRuntime, IngressRoute, RuntimeDeployment, RuntimeHealth,
    RuntimeLog, RuntimeObservation,
};
use ignitify_db::{RemoteServerConnection, RemoteServersRepository};
use ignitify_domain::{ServiceSpec, is_digest_image_reference};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::{io::AsyncWriteExt, process::Command, time::timeout};
use yaml_rust2::YamlLoader;
use zeroize::Zeroizing;

const SSH_TIMEOUT: Duration = Duration::from_secs(45);
const PROXY_NETWORK: &str = "ignitify-proxy";
const MANAGED_LABEL: &str = "com.ignitify.managed";
const SERVICE_LABEL: &str = "com.ignitify.service-id";
const GENERATION_LABEL: &str = "com.ignitify.generation";

#[derive(Clone)]
pub struct SshRuntime {
    servers: RemoteServersRepository,
    cipher: Arc<AgeCipher>,
}

impl SshRuntime {
    pub fn new(servers: RemoteServersRepository, cipher: Arc<AgeCipher>) -> Self {
        Self { servers, cipher }
    }

    fn runtime_ref(destination_id: &str, deployment: &RuntimeDeployment) -> String {
        format!(
            "ignitify-remote-{destination_id}-service-{}-g{}",
            deployment.service_id, deployment.generation
        )
    }

    async fn connection(&self, destination_id: &str) -> Result<RemoteSecrets, ControlError> {
        let connection = self
            .servers
            .connection(destination_id)
            .await
            .map_err(|_| ControlError::Runtime)?
            .ok_or(ControlError::Runtime)?;
        let private_key = self
            .cipher
            .decrypt(&connection.private_key_ciphertext)
            .map_err(|_| ControlError::Runtime)?;
        let known_hosts = self
            .cipher
            .decrypt(&connection.known_hosts_ciphertext)
            .map_err(|_| ControlError::Runtime)?;
        Ok(RemoteSecrets {
            connection,
            private_key,
            known_hosts,
        })
    }

    async fn execute(
        &self,
        secrets: &RemoteSecrets,
        script: String,
    ) -> Result<RemoteOutput, ControlError> {
        let directory = tempfile_directory();
        tokio::fs::create_dir(&directory)
            .await
            .map_err(|_| ControlError::Runtime)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            tokio::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
                .await
                .map_err(|_| ControlError::Runtime)?;
        }
        let key_path = directory.join("id_key");
        let known_hosts_path = directory.join("known_hosts");
        let result = async {
            write_secret(&key_path, &terminated_key(&secrets.private_key)).await?;
            write_secret(&known_hosts_path, &secrets.known_hosts).await?;
            let key = key_path.to_string_lossy().into_owned();
            let known_hosts = known_hosts_path.to_string_lossy().into_owned();
            let global_known_hosts = if cfg!(windows) { "NUL" } else { "/dev/null" };
            let port = secrets.connection.port.to_string();
            let user_known_hosts = format!("UserKnownHostsFile={known_hosts}");
            let global_known_hosts = format!("GlobalKnownHostsFile={global_known_hosts}");
            let target = format!(
                "{}@{}",
                secrets.connection.username, secrets.connection.host
            );
            let mut child = Command::new("ssh")
                .kill_on_drop(true)
                .args([
                    "-F",
                    "none",
                    "-i",
                    key.as_str(),
                    "-o",
                    "IdentitiesOnly=yes",
                    "-o",
                    "PasswordAuthentication=no",
                    "-o",
                    "BatchMode=yes",
                    "-o",
                    "StrictHostKeyChecking=yes",
                    "-o",
                    user_known_hosts.as_str(),
                    "-o",
                    global_known_hosts.as_str(),
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
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|_| ControlError::Runtime)?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(script.as_bytes())
                    .await
                    .map_err(|_| ControlError::Runtime)?;
            }
            let output = timeout(SSH_TIMEOUT, child.wait_with_output())
                .await
                .map_err(|_| ControlError::Runtime)?
                .map_err(|_| ControlError::Runtime)?;
            if !output.status.success() && is_authentication_failure(&output.stderr) {
                let _ = self
                    .servers
                    .record_authentication_failure(&secrets.connection.id)
                    .await;
            }
            Ok(RemoteOutput {
                success: output.status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        }
        .await;
        let _ = tokio::fs::remove_dir_all(&directory).await;
        result
    }

    fn render_base(deployment: &RuntimeDeployment) -> Result<String, ControlError> {
        match &deployment.spec {
            ServiceSpec::Image {
                image_reference,
                healthcheck,
                ..
            } => {
                let image_reference = deployment
                    .local_image_id
                    .as_deref()
                    .unwrap_or(image_reference);
                if !is_digest_image_reference(image_reference) {
                    return Err(ControlError::Policy(
                        "remote deployments require a registry-backed immutable image",
                    ));
                }
                let healthcheck = healthcheck.as_ref().map(|args| {
                    let values = args
                        .iter()
                        .map(|value| format!("\"{}\"", yaml_quote(value)))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("\n    healthcheck:\n      test: [{values}]\n")
                });
                Ok(format!(
                    "services:\n  app:\n    image: \"{}\"\n{}",
                    yaml_quote(image_reference),
                    healthcheck.unwrap_or_default()
                ))
            }
            ServiceSpec::Compose { yaml, .. } => Ok(yaml.clone()),
        }
    }

    fn exposed_service(deployment: &RuntimeDeployment) -> &str {
        match &deployment.spec {
            ServiceSpec::Image { .. } => "app",
            ServiceSpec::Compose {
                exposed_service, ..
            } => exposed_service,
        }
    }

    fn render_override(
        deployment: &RuntimeDeployment,
        routes: &[IngressRoute],
    ) -> Result<String, ControlError> {
        let mut labels = vec![
            format!("      {MANAGED_LABEL}: \"true\""),
            format!("      {SERVICE_LABEL}: \"{}\"", deployment.service_id),
            format!("      {GENERATION_LABEL}: \"{}\"", deployment.generation),
        ];
        for route in routes {
            for (key, value) in &route.labels {
                labels.push(format!(
                    "      \"{}\": \"{}\"",
                    yaml_quote(key),
                    yaml_quote(value)
                ));
            }
        }
        let network = if routes.is_empty() {
            String::new()
        } else {
            format!(
                "    networks:\n      - {PROXY_NETWORK}\nnetworks:\n  {PROXY_NETWORK}:\n    external: true\n"
            )
        };
        let volumes = Self::canonical_volume_names(deployment)?;
        let volumes = (!volumes.is_empty()).then(|| {
            let entries = volumes
                .into_iter()
                .map(|(name, value)| format!("  \"{}\":\n    name: {value}", yaml_quote(&name)))
                .collect::<Vec<_>>()
                .join("\n");
            format!("volumes:\n{entries}\n")
        });
        Ok(format!(
            "services:\n  {}:\n    labels:\n{}\n{}{}",
            Self::exposed_service(deployment),
            labels.join("\n"),
            network,
            volumes.unwrap_or_default(),
        ))
    }

    fn canonical_volume_names(
        deployment: &RuntimeDeployment,
    ) -> Result<Vec<(String, String)>, ControlError> {
        let ServiceSpec::Compose { yaml, .. } = &deployment.spec else {
            return Ok(Vec::new());
        };
        let documents = YamlLoader::load_from_str(yaml)
            .map_err(|_| ControlError::Policy("invalid Compose YAML"))?;
        let Some(root) = documents.first() else {
            return Ok(Vec::new());
        };
        let Some(volumes) = root["volumes"].as_hash() else {
            return Ok(Vec::new());
        };
        Ok(volumes
            .keys()
            .filter_map(|name| name.as_str())
            .map(|name| {
                let mut digest = Sha256::new();
                digest.update(name.as_bytes());
                let digest = format!("{:x}", digest.finalize());
                (
                    name.to_owned(),
                    format!("ignitify-{}-{}", deployment.service_id, &digest[..24]),
                )
            })
            .collect())
    }

    fn compose_script(
        deploy_path: &str,
        deployment: &RuntimeDeployment,
        environment: &[String],
        routes: &[IngressRoute],
        action: &str,
    ) -> Result<String, ControlError> {
        let destination = deployment
            .deployment_destination_id
            .as_deref()
            .ok_or(ControlError::Runtime)?;
        let project = Self::runtime_ref(destination, deployment);
        let stage = format!(
            "$IGNITIFY_ROOT/releases/{}/{}",
            deployment.service_id, deployment.generation
        );
        let base = BASE64.encode(Self::render_base(deployment)?);
        let env = BASE64.encode(environment.join("\n"));
        let override_file = BASE64.encode(Self::render_override(deployment, routes)?);
        Ok(format!(
            r#"set -eu
command -v docker >/dev/null 2>&1
docker compose version >/dev/null 2>&1
IGNITIFY_ROOT={root}
STAGE="{stage}"
PROJECT={project}
mkdir -p "$STAGE"
printf '%s' '{base}' | base64 -d > "$STAGE/compose.yaml"
printf '%s' '{env}' | base64 -d > "$STAGE/ignitify.env"
printf '%s' '{override_file}' | base64 -d > "$STAGE/ignitify.override.yaml"
chmod 700 "$STAGE"
chmod 600 "$STAGE/compose.yaml" "$STAGE/ignitify.env" "$STAGE/ignitify.override.yaml"
cd "$STAGE"
docker compose --project-directory "$STAGE" --project-name "$PROJECT" --file compose.yaml --env-file ignitify.env --file ignitify.override.yaml config >/dev/null
{network_check}{action}
"#,
            root = shell_quote(deploy_path),
            stage = stage,
            project = shell_quote(&project),
            base = base,
            env = env,
            override_file = override_file,
            network_check = if routes.is_empty() {
                ""
            } else {
                "docker network inspect ignitify-proxy >/dev/null\n"
            },
            action = action,
        ))
    }

    fn existing_compose_script(
        deploy_path: &str,
        deployment: &RuntimeDeployment,
        action: &str,
    ) -> Result<String, ControlError> {
        let destination = deployment
            .deployment_destination_id
            .as_deref()
            .ok_or(ControlError::Runtime)?;
        let project = Self::runtime_ref(destination, deployment);
        let stage = format!(
            "$IGNITIFY_ROOT/releases/{}/{}",
            deployment.service_id, deployment.generation
        );
        Ok(format!(
            r#"set -eu
command -v docker >/dev/null 2>&1
docker compose version >/dev/null 2>&1
IGNITIFY_ROOT={root}
STAGE="{stage}"
PROJECT={project}
test -d "$STAGE"
cd "$STAGE"
{action}
"#,
            root = shell_quote(deploy_path),
            stage = stage,
            project = shell_quote(&project),
            action = action,
        ))
    }

    async fn start_remote(
        &self,
        deployment: &RuntimeDeployment,
        environment: Vec<String>,
    ) -> Result<String, ControlError> {
        let destination = deployment
            .deployment_destination_id
            .as_deref()
            .ok_or(ControlError::Runtime)?;
        let secrets = self.connection(destination).await?;
        let script = Self::compose_script(
            &secrets.connection.deploy_path,
            deployment,
            &environment,
            &[],
            "docker compose --project-directory \"$STAGE\" --project-name \"$PROJECT\" --file compose.yaml --env-file ignitify.env --file ignitify.override.yaml up --detach --no-build --remove-orphans",
        )?;
        let output = self.execute(&secrets, script).await?;
        if output.success {
            Ok(Self::runtime_ref(destination, deployment))
        } else {
            Err(ControlError::Runtime)
        }
    }

    async fn inspect_remote(
        &self,
        deployment: &RuntimeDeployment,
        runtime_ref: &str,
    ) -> Result<RuntimeObservation, ControlError> {
        if runtime_ref != self.runtime_ref(deployment) {
            return Ok(RuntimeObservation {
                owned: false,
                running: false,
                healthy: None,
                health_failing: false,
            });
        }
        let destination = deployment
            .deployment_destination_id
            .as_deref()
            .ok_or(ControlError::Runtime)?;
        let secrets = self.connection(destination).await?;
        let script = Self::existing_compose_script(
            &secrets.connection.deploy_path,
            deployment,
            "docker compose --project-directory \"$STAGE\" --project-name \"$PROJECT\" --file compose.yaml --env-file ignitify.env --file ignitify.override.yaml ps --all --format json",
        )?;
        let output = self.execute(&secrets, script).await?;
        if !output.success {
            return Err(ControlError::Runtime);
        }
        Ok(parse_observation(
            &output.stdout,
            Self::exposed_service(deployment),
        ))
    }

    async fn stop_remote(
        &self,
        runtime_ref: &str,
        service_id: &str,
        generation: i64,
    ) -> Result<bool, ControlError> {
        let Some((destination, embedded_service, embedded_generation)) =
            parse_runtime_ref(runtime_ref)
        else {
            return Ok(false);
        };
        if embedded_service != service_id || embedded_generation != generation {
            return Ok(false);
        }
        let secrets = self.connection(destination).await?;
        let root = shell_quote(&secrets.connection.deploy_path);
        let stage = format!("$IGNITIFY_ROOT/releases/{service_id}/{generation}");
        let project = shell_quote(runtime_ref);
        let script = format!(
            "set -eu\nIGNITIFY_ROOT={root}\nSTAGE={stage}\nPROJECT={project}\ncd \"$STAGE\"\ndocker compose --project-directory \"$STAGE\" --project-name \"$PROJECT\" --file compose.yaml --env-file ignitify.env --file ignitify.override.yaml down --remove-orphans\nrm -rf \"$STAGE\"\n"
        );
        let output = self.execute(&secrets, script).await?;
        Ok(output.success)
    }

    async fn logs_remote(
        &self,
        runtime_ref: &str,
        since: i64,
    ) -> Result<Vec<RuntimeLog>, ControlError> {
        let Some((destination, service_id, generation)) = parse_runtime_ref(runtime_ref) else {
            return Err(ControlError::Runtime);
        };
        let secrets = self.connection(destination).await?;
        let root = shell_quote(&secrets.connection.deploy_path);
        let stage = format!("$IGNITIFY_ROOT/releases/{service_id}/{generation}");
        let project = shell_quote(runtime_ref);
        let script = format!(
            "set -eu\nIGNITIFY_ROOT={root}\nSTAGE={stage}\nPROJECT={project}\ncd \"$STAGE\"\ndocker compose --project-directory \"$STAGE\" --project-name \"$PROJECT\" --file compose.yaml --env-file ignitify.env --file ignitify.override.yaml logs --timestamps --since {since}\n"
        );
        let output = self.execute(&secrets, script).await?;
        if !output.success {
            return Err(ControlError::Runtime);
        }
        Ok(parse_logs(&output.stdout, &output.stderr))
    }

    async fn reconcile_remote(
        &self,
        deployment: &RuntimeDeployment,
        environment: Vec<String>,
        routes: Vec<IngressRoute>,
    ) -> Result<bool, ControlError> {
        let destination = deployment
            .deployment_destination_id
            .as_deref()
            .ok_or(ControlError::Runtime)?;
        let secrets = self.connection(destination).await?;
        let script = Self::compose_script(
            &secrets.connection.deploy_path,
            deployment,
            &environment,
            &routes,
            "docker compose --project-directory \"$STAGE\" --project-name \"$PROJECT\" --file compose.yaml --env-file ignitify.env --file ignitify.override.yaml up --detach --no-build --remove-orphans",
        )?;
        Ok(self.execute(&secrets, script).await?.success)
    }
}

impl RuntimeHealth for SshRuntime {
    fn ready(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
        Box::pin(std::future::ready(true))
    }
}

impl ImageRuntime for SshRuntime {
    fn runtime_ref(&self, deployment: &RuntimeDeployment) -> String {
        Self::runtime_ref(
            deployment
                .deployment_destination_id
                .as_deref()
                .unwrap_or("unavailable"),
            deployment,
        )
    }

    async fn start(
        &self,
        deployment: &RuntimeDeployment,
        environment: Vec<String>,
    ) -> Result<String, ControlError> {
        self.start_remote(deployment, environment).await
    }

    async fn inspect(
        &self,
        deployment: &RuntimeDeployment,
        runtime_ref: &str,
    ) -> Result<RuntimeObservation, ControlError> {
        self.inspect_remote(deployment, runtime_ref).await
    }

    async fn stop(
        &self,
        runtime_ref: &str,
        service_id: &str,
        generation: i64,
    ) -> Result<bool, ControlError> {
        self.stop_remote(runtime_ref, service_id, generation).await
    }

    async fn logs(&self, runtime_ref: &str, since: i64) -> Result<Vec<RuntimeLog>, ControlError> {
        self.logs_remote(runtime_ref, since).await
    }

    async fn reconcile_routes(
        &self,
        deployment: &RuntimeDeployment,
        _runtime_ref: &str,
        environment: Vec<String>,
        routes: Vec<IngressRoute>,
    ) -> Result<bool, ControlError> {
        self.reconcile_remote(deployment, environment, routes).await
    }
}

struct RemoteSecrets {
    connection: RemoteServerConnection,
    private_key: Zeroizing<Vec<u8>>,
    known_hosts: Zeroizing<Vec<u8>>,
}

struct RemoteOutput {
    success: bool,
    stdout: String,
    #[allow(dead_code)]
    stderr: String,
}

fn tempfile_directory() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("ignitify-remote-{}", uuid::Uuid::new_v4()))
}

fn terminated_key(key: &[u8]) -> Zeroizing<Vec<u8>> {
    let mut value = Zeroizing::new(key.to_vec());
    if !value.ends_with(b"\n") {
        value.push(b'\n');
    }
    value
}

async fn write_secret(path: &Path, bytes: &[u8]) -> Result<(), ControlError> {
    tokio::fs::write(path, bytes)
        .await
        .map_err(|_| ControlError::Runtime)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .await
            .map_err(|_| ControlError::Runtime)?;
    }
    Ok(())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn yaml_quote(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\n', '\r'], " ")
}

fn is_authentication_failure(stderr: &[u8]) -> bool {
    String::from_utf8_lossy(stderr)
        .to_ascii_lowercase()
        .contains("permission denied")
}

fn parse_runtime_ref(value: &str) -> Option<(&str, &str, i64)> {
    let value = value.strip_prefix("ignitify-remote-")?;
    let (value, generation) = value.rsplit_once("-g")?;
    let generation = generation.parse().ok()?;
    let (destination, service) = value.split_once("-service-")?;
    Some((destination, service, generation))
}

#[derive(Debug, Deserialize)]
struct ComposeStatus {
    #[serde(rename = "Service", alias = "service")]
    service: Option<String>,
    #[serde(rename = "State", alias = "state")]
    state: Option<String>,
    #[serde(rename = "Health", alias = "health")]
    health: Option<String>,
    #[serde(rename = "Status", alias = "status")]
    status: Option<String>,
}

fn parse_observation(output: &str, exposed_service: &str) -> RuntimeObservation {
    let rows = serde_json::from_str::<Vec<ComposeStatus>>(output).unwrap_or_else(|_| {
        output
            .lines()
            .filter_map(|line| serde_json::from_str::<ComposeStatus>(line).ok())
            .collect()
    });
    let rows = rows
        .into_iter()
        .filter(|row| {
            row.service
                .as_deref()
                .is_none_or(|service| service == exposed_service)
        })
        .collect::<Vec<_>>();
    let running = !rows.is_empty()
        && rows.iter().all(|row| {
            row.state.as_deref().is_some_and(|state| state == "running")
                || row
                    .status
                    .as_deref()
                    .is_some_and(|status| status.contains("Up"))
        });
    let health_failing = rows.iter().any(|row| {
        row.health
            .as_deref()
            .is_some_and(|health| health.contains("unhealthy"))
            || row
                .status
                .as_deref()
                .is_some_and(|status| status.contains("unhealthy"))
    });
    let has_health = rows.iter().any(|row| row.health.is_some());
    RuntimeObservation {
        owned: true,
        running,
        healthy: has_health.then_some(running && !health_failing),
        health_failing,
    }
}

fn parse_logs(stdout: &str, stderr: &str) -> Vec<RuntimeLog> {
    let mut logs = stdout
        .lines()
        .map(|line| RuntimeLog {
            stream: "stdout".to_owned(),
            line: line.to_owned(),
        })
        .collect::<Vec<_>>();
    logs.extend(stderr.lines().map(|line| RuntimeLog {
        stream: "stderr".to_owned(),
        line: line.to_owned(),
    }));
    logs
}

#[cfg(test)]
mod tests {
    use super::{
        SshRuntime, is_authentication_failure, parse_observation, parse_runtime_ref, shell_quote,
        terminated_key,
    };
    use ignitify_control_plane::RuntimeDeployment;
    use ignitify_domain::{DeploymentId, ServiceId, ServiceSpec};

    fn deployment() -> RuntimeDeployment {
        RuntimeDeployment {
            id: DeploymentId::new("11111111-1111-1111-1111-111111111111").unwrap(),
            service_id: ServiceId::new("22222222-2222-2222-2222-222222222222").unwrap(),
            generation: 7,
            spec: ServiceSpec::Image {
                image_reference:
                    "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_owned(),
                internal_port: Some(80),
                healthcheck: None,
            },
            local_image_id: None,
            deployment_destination_id: Some("33333333-3333-3333-3333-333333333333".to_owned()),
        }
    }

    #[test]
    fn runtime_reference_is_destination_scoped() {
        let value = format!(
            "ignitify-remote-{}-service-{}-g7",
            "33333333-3333-3333-3333-333333333333", "22222222-2222-2222-2222-222222222222"
        );
        assert_eq!(
            parse_runtime_ref(&value),
            Some((
                "33333333-3333-3333-3333-333333333333",
                "22222222-2222-2222-2222-222222222222",
                7
            ))
        );
    }

    #[test]
    fn image_release_renders_digest_only_compose() {
        assert!(
            SshRuntime::render_base(&deployment())
                .unwrap()
                .contains("nginx@sha256:")
        );
    }

    #[test]
    fn image_release_uses_published_remote_builder_digest() {
        let mut release_deployment = deployment();
        release_deployment.local_image_id = Some(format!(
            "registry.example.com/ignitify/builds:release@sha256:{}",
            "b".repeat(64)
        ));
        let release = SshRuntime::render_base(&release_deployment).unwrap();
        assert!(release.contains("registry.example.com/ignitify/builds:release@sha256:"));
        assert!(!release.contains("nginx@sha256:"));
    }

    #[test]
    fn shell_quote_does_not_expand_values() {
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }

    #[test]
    fn private_key_termination_uses_a_zeroizing_buffer() {
        let value = terminated_key(b"private-key");
        assert_eq!(value.as_slice(), b"private-key\n");
    }

    #[test]
    fn remote_authentication_failure_detection_is_safe() {
        assert!(is_authentication_failure(b"Permission denied (publickey)."));
        assert!(!is_authentication_failure(b"Host key verification failed."));
    }

    #[test]
    fn compose_script_expands_release_root_at_execution_time() {
        let script =
            SshRuntime::compose_script("/srv/ignitify", &deployment(), &[], &[], "true").unwrap();
        assert!(script.contains("IGNITIFY_ROOT='/srv/ignitify'"));
        assert!(script.contains("STAGE=\"$IGNITIFY_ROOT/releases/"));
        assert!(!script.contains("remote-deploy-root"));
    }

    #[test]
    fn observation_accepts_compose_json_arrays() {
        let observation = parse_observation(
            r#"[{"Service":"app","State":"running","Health":"healthy"}]"#,
            "app",
        );
        assert!(observation.owned);
        assert!(observation.running);
        assert_eq!(observation.healthy, Some(true));
    }

    #[test]
    fn compose_release_uses_stable_named_volumes_across_generations() {
        let mut release = deployment();
        release.spec = ServiceSpec::compose(
            "services:\n  web:\n    image: nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n    volumes:\n      - data:/var/lib/app\nvolumes:\n  data: {}\n",
            "web",
            Some(80),
        )
        .unwrap();
        let first = SshRuntime::render_override(&release, &[]).unwrap();
        release.generation += 1;
        let second = SshRuntime::render_override(&release, &[]).unwrap();
        assert!(first.contains("volumes:\n  \"data\":"));
        let first_volume = first
            .lines()
            .find(|line| line.trim_start().starts_with("name:"))
            .unwrap();
        let second_volume = second
            .lines()
            .find(|line| line.trim_start().starts_with("name:"))
            .unwrap();
        assert_eq!(first_volume, second_volume);
    }
}
