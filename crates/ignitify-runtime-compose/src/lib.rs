use std::{
    env,
    path::{Path, PathBuf},
    process::Stdio,
};

use ignitify_control_plane::{
    Error as ControlError, ImageRuntime, IngressRoute, RuntimeDeployment, RuntimeHealth,
    RuntimeLog, RuntimeObservation,
};
use ignitify_domain::ServiceSpec;
use serde_json::Value;
use tokio::{fs, process::Command};
mod error;
mod policy;
mod render;

pub use error::Error;
pub use policy::validate_submission_yaml;
use policy::{ensure_exposed_service, preflight_yaml, validate_canonical};
use render::{canonical_volume_names, render_override};

type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
pub struct ComposeRuntime {
    docker: PathBuf,
    root: PathBuf,
}

impl ComposeRuntime {
    pub fn from_environment() -> Result<Self> {
        Self::from_paths(
            env::var_os("IGNITIFY_DOCKER_BIN").map(PathBuf::from),
            env::var_os("IGNITIFY_COMPOSE_ROOT").map(PathBuf::from),
        )
    }

    pub fn from_paths(docker: Option<PathBuf>, root: Option<PathBuf>) -> Result<Self> {
        Self::new(
            docker.unwrap_or_else(default_docker_binary),
            root.unwrap_or_else(|| PathBuf::from("data/compose")),
        )
    }

    pub fn new(docker: impl Into<PathBuf>, root: impl Into<PathBuf>) -> Result<Self> {
        let docker = docker.into();
        if !docker.is_absolute() {
            return Err(Error::InvalidDockerPath);
        }
        Ok(Self {
            docker,
            root: root.into(),
        })
    }

    pub async fn ready(&self) -> bool {
        self.command(["version", "--format", "{{.Server.Version}}"])
            .output()
            .await
            .is_ok_and(|output| output.status.success())
    }

    fn project_name(deployment: &RuntimeDeployment) -> String {
        format!(
            "ignitify-{}-g{}",
            deployment.service_id, deployment.generation
        )
    }

    fn stage(&self, deployment: &RuntimeDeployment) -> PathBuf {
        self.root
            .join(deployment.service_id.to_string())
            .join(deployment.generation.to_string())
    }

    fn command<I, S>(&self, args: I) -> Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let mut command = Command::new(&self.docker);
        command
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    async fn prepare_stage(
        &self,
        deployment: &RuntimeDeployment,
        yaml: &str,
        environment: &[String],
    ) -> Result<PathBuf> {
        preflight_yaml(yaml)?;
        let stage = self.stage(deployment);
        let service_directory = stage
            .parent()
            .ok_or(Error::Policy("invalid Compose stage path"))?;
        fs::create_dir_all(&stage).await.map_err(Error::Io)?;
        restrict_directory(&self.root).await?;
        restrict_directory(service_directory).await?;
        restrict_directory(&stage).await?;
        if environment.iter().any(|entry| {
            entry.split_once('=').is_none_or(|(key, value)| {
                key.is_empty() || key.contains(['=', '\r', '\n']) || value.contains(['\r', '\n'])
            })
        }) {
            return Err(Error::Policy(
                "Compose environment contains unsupported newline or key",
            ));
        }
        write_restricted(&stage.join("compose.yaml"), yaml.as_bytes()).await?;
        write_restricted(
            &stage.join("ignitify.env"),
            environment.join("\n").as_bytes(),
        )
        .await?;
        Ok(stage)
    }

    fn stage_from_runtime_ref(
        &self,
        runtime_ref: &str,
        service_id: &str,
        generation: i64,
    ) -> Option<PathBuf> {
        (runtime_ref == format!("ignitify-{service_id}-g{generation}"))
            .then(|| self.root.join(service_id).join(generation.to_string()))
    }

    async fn canonicalize(
        &self,
        stage: &Path,
        project: &str,
        override_file: bool,
    ) -> Result<Value> {
        let mut args = compose_args(stage, project);
        if override_file {
            args.extend([
                "--file".to_owned(),
                stage.join("ignitify.override.yaml").display().to_string(),
            ]);
        }
        args.extend([
            "config".to_owned(),
            "--format".to_owned(),
            "json".to_owned(),
        ]);
        let output = self
            .command(args)
            .current_dir(stage)
            .output()
            .await
            .map_err(Error::Io)?;
        if !output.status.success() {
            return Err(Error::CommandFailed(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }
        let value: Value =
            serde_json::from_slice(&output.stdout).map_err(|_| Error::InvalidCanonicalConfig)?;
        validate_canonical(&value, override_file)?;
        Ok(value)
    }

    async fn up(&self, stage: &Path, project: &str, override_file: bool) -> Result<()> {
        let output = self
            .run_compose(
                stage,
                project,
                override_file,
                [
                    "up".to_owned(),
                    "--detach".to_owned(),
                    "--no-build".to_owned(),
                    "--remove-orphans".to_owned(),
                ],
            )
            .await?;
        if output.status.success() {
            Ok(())
        } else {
            Err(Error::CommandFailed(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ))
        }
    }

    async fn down(&self, stage: &Path, project: &str) {
        let _ = self
            .run_compose(
                stage,
                project,
                true,
                ["down".to_owned(), "--remove-orphans".to_owned()],
            )
            .await;
    }

    async fn run_compose<I>(
        &self,
        stage: &Path,
        project: &str,
        override_file: bool,
        command: I,
    ) -> Result<std::process::Output>
    where
        I: IntoIterator<Item = String>,
    {
        let mut args = compose_args(stage, project);
        if override_file {
            args.extend([
                "--file".to_owned(),
                stage.join("ignitify.override.yaml").display().to_string(),
            ]);
        }
        args.extend(command);
        self.command(args)
            .current_dir(stage)
            .output()
            .await
            .map_err(Error::Io)
    }

    async fn write_override(
        &self,
        stage: &Path,
        deployment: &RuntimeDeployment,
        routes: &[IngressRoute],
        canonical: &Value,
    ) -> Result<()> {
        let content = render_override(
            deployment,
            routes,
            canonical_volume_names(deployment, canonical),
        )?;
        write_restricted(&stage.join("ignitify.override.yaml"), content.as_bytes()).await
    }

    async fn ps(&self, stage: &Path, project: &str) -> Result<String> {
        let mut args = compose_args(stage, project);
        args.extend([
            "ps".to_owned(),
            "--all".to_owned(),
            "--format".to_owned(),
            "json".to_owned(),
        ]);
        let output = self
            .command(args)
            .current_dir(stage)
            .output()
            .await
            .map_err(Error::Io)?;
        if !output.status.success() {
            return Err(Error::CommandFailed(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

fn default_docker_binary() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("ProgramFiles")
            .map(PathBuf::from)
            .map(|path| path.join("Docker\\Docker\\resources\\bin\\docker.exe"))
            .unwrap_or_else(|| {
                PathBuf::from("C:/Program Files/Docker/Docker/resources/bin/docker.exe")
            })
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/usr/bin/docker")
    }
}

impl RuntimeHealth for ComposeRuntime {
    fn ready(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
        Box::pin(async move { self.ready().await })
    }
}

impl ImageRuntime for ComposeRuntime {
    fn runtime_ref(&self, deployment: &RuntimeDeployment) -> String {
        Self::project_name(deployment)
    }

    async fn start(
        &self,
        deployment: &RuntimeDeployment,
        environment: Vec<String>,
    ) -> std::result::Result<String, ControlError> {
        let ServiceSpec::Compose {
            yaml,
            exposed_service,
            ..
        } = &deployment.spec
        else {
            return Err(ControlError::Runtime);
        };
        let stage = self.stage(deployment);
        let project = Self::project_name(deployment);
        let mut up_attempted = false;
        let result = async {
            self.prepare_stage(deployment, yaml, &environment).await?;
            let canonical = self.canonicalize(&stage, &project, false).await?;
            ensure_exposed_service(&canonical, exposed_service)?;
            self.write_override(&stage, deployment, &[], &canonical)
                .await?;
            self.canonicalize(&stage, &project, true).await?;
            up_attempted = true;
            self.up(&stage, &project, true).await
        }
        .await;
        if let Err(error) = result {
            if up_attempted {
                self.down(&stage, &project).await;
            }
            let _ = fs::remove_dir_all(&stage).await;
            return Err(control_error(error));
        }
        Ok(project)
    }

    async fn inspect(
        &self,
        deployment: &RuntimeDeployment,
        runtime_ref: &str,
    ) -> std::result::Result<RuntimeObservation, ControlError> {
        if runtime_ref != Self::project_name(deployment) {
            return Ok(RuntimeObservation {
                owned: false,
                running: false,
                healthy: None,
                health_failing: false,
            });
        }
        let stage = self.stage(deployment);
        if fs::metadata(&stage).await.is_err() {
            return Ok(RuntimeObservation {
                owned: true,
                running: false,
                healthy: None,
                health_failing: false,
            });
        }
        let output = self
            .ps(&stage, runtime_ref)
            .await
            .map_err(|_| ControlError::Runtime)?;
        let ServiceSpec::Compose {
            exposed_service, ..
        } = &deployment.spec
        else {
            return Err(ControlError::Runtime);
        };
        let records = parse_ps(&output);
        let statuses = service_statuses(&records, exposed_service);
        let running = !statuses.is_empty()
            && statuses
                .iter()
                .all(|value| value.contains("up") || value.contains("running"));
        let health_failing = statuses.iter().any(|value| value.contains("unhealthy"));
        let has_health_state = statuses.iter().any(|value| {
            value.contains("healthy") || value.contains("starting") || value.contains("unhealthy")
        });
        let healthy = has_health_state.then(|| {
            running
                && !statuses
                    .iter()
                    .any(|value| value.contains("starting") || value.contains("unhealthy"))
        });
        Ok(RuntimeObservation {
            owned: true,
            running,
            healthy,
            health_failing,
        })
    }

    async fn stop(
        &self,
        runtime_ref: &str,
        service_id: &str,
        generation: i64,
    ) -> std::result::Result<bool, ControlError> {
        let Some(stage) = self.stage_from_runtime_ref(runtime_ref, service_id, generation) else {
            return Ok(false);
        };
        if fs::metadata(&stage).await.is_err() {
            return Ok(true);
        }
        let output = self
            .run_compose(
                &stage,
                runtime_ref,
                true,
                ["down".to_owned(), "--remove-orphans".to_owned()],
            )
            .await
            .map_err(control_error)?;
        if !output.status.success() {
            return Err(ControlError::Runtime);
        }
        let _ = fs::remove_dir_all(stage).await;
        Ok(true)
    }

    async fn logs(
        &self,
        runtime_ref: &str,
        since: i64,
    ) -> std::result::Result<Vec<RuntimeLog>, ControlError> {
        let Some((service, generation)) = runtime_ref
            .strip_prefix("ignitify-")
            .and_then(|value| value.rsplit_once("-g"))
        else {
            return Err(ControlError::Runtime);
        };
        let Ok(generation) = generation.parse::<i64>() else {
            return Err(ControlError::Runtime);
        };
        let Some(stage) = self.stage_from_runtime_ref(runtime_ref, service, generation) else {
            return Err(ControlError::Runtime);
        };
        let mut args = compose_args(&stage, runtime_ref);
        args.extend([
            "logs".to_owned(),
            "--timestamps".to_owned(),
            "--since".to_owned(),
            since.to_string(),
        ]);
        let output = self
            .command(args)
            .current_dir(&stage)
            .output()
            .await
            .map_err(|_| ControlError::Runtime)?;
        if !output.status.success() {
            return Err(ControlError::Runtime);
        }
        Ok(output_logs(&output))
    }

    async fn reconcile_routes(
        &self,
        deployment: &RuntimeDeployment,
        _runtime_ref: &str,
        environment: Vec<String>,
        routes: Vec<IngressRoute>,
    ) -> std::result::Result<bool, ControlError> {
        let ServiceSpec::Compose {
            yaml,
            exposed_service,
            ..
        } = &deployment.spec
        else {
            return Err(ControlError::Runtime);
        };
        let stage = self.stage(deployment);
        let project = Self::project_name(deployment);
        let result = async {
            self.prepare_stage(deployment, yaml, &environment).await?;
            let base = self.canonicalize(&stage, &project, false).await?;
            self.write_override(&stage, deployment, &routes, &base)
                .await?;
            let canonical = self.canonicalize(&stage, &project, true).await?;
            ensure_exposed_service(&canonical, exposed_service)?;
            self.up(&stage, &project, true).await
        }
        .await;
        result.map_err(control_error)?;
        Ok(true)
    }
}

fn control_error(error: Error) -> ControlError {
    match error {
        Error::Policy(message) => ControlError::Policy(message),
        Error::InvalidDockerPath
        | Error::UnsupportedSpec
        | Error::CommandFailed(_)
        | Error::InvalidCanonicalConfig
        | Error::Io(_) => ControlError::Runtime,
    }
}

fn output_logs(output: &std::process::Output) -> Vec<RuntimeLog> {
    let mut logs = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| RuntimeLog {
            stream: "stdout".to_owned(),
            line: line.to_owned(),
        })
        .collect::<Vec<_>>();
    logs.extend(
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .map(|line| RuntimeLog {
                stream: "stderr".to_owned(),
                line: line.to_owned(),
            }),
    );
    logs
}

fn compose_args(stage: &Path, project: &str) -> Vec<String> {
    vec![
        "compose".to_owned(),
        "--project-directory".to_owned(),
        stage.display().to_string(),
        "--project-name".to_owned(),
        project.to_owned(),
        "--file".to_owned(),
        stage.join("compose.yaml").display().to_string(),
        "--env-file".to_owned(),
        stage.join("ignitify.env").display().to_string(),
    ]
}

async fn restrict_directory(_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(_path, std::fs::Permissions::from_mode(0o700))
            .await
            .map_err(Error::Io)?;
    }
    Ok(())
}

async fn write_restricted(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::write(path, bytes).await.map_err(Error::Io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .await
            .map_err(Error::Io)?;
    }
    Ok(())
}

fn parse_ps(output: &str) -> Vec<Value> {
    serde_json::from_str(output).unwrap_or_else(|_| {
        output
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    })
}

fn status(value: &Value) -> String {
    value
        .get("State")
        .or_else(|| value.get("Status"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn service_statuses(records: &[Value], exposed_service: &str) -> Vec<String> {
    records
        .iter()
        .filter(|record| {
            record
                .get("Service")
                .and_then(Value::as_str)
                .is_some_and(|service| service == exposed_service)
        })
        .map(status)
        .collect()
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
