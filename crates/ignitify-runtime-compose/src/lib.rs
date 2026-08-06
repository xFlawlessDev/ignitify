use std::{
    collections::{BTreeSet, HashSet},
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
use yaml_rust2::{
    Yaml, YamlLoader,
    parser::{Event, EventReceiver, Parser},
};

const MAX_YAML_BYTES: usize = 1024 * 1024;
const MAX_DEPTH: usize = 64;
const MAX_SERVICES: usize = 100;
const PROXY_NETWORK: &str = "ignitify-proxy";
const MANAGED_LABEL: &str = "com.ignitify.managed";
const SERVICE_LABEL: &str = "com.ignitify.service-id";
const GENERATION_LABEL: &str = "com.ignitify.generation";
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
            docker.unwrap_or_else(|| PathBuf::from("/usr/bin/docker")),
            root.unwrap_or_else(|| PathBuf::from("/var/lib/ignitify/compose")),
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
    ) -> Result<()> {
        let ServiceSpec::Compose {
            exposed_service, ..
        } = &deployment.spec
        else {
            return Err(Error::UnsupportedSpec);
        };
        let mut labels = vec![
            format!("      {MANAGED_LABEL}: \"true\""),
            format!("      {SERVICE_LABEL}: \"{}\"", deployment.service_id),
            format!("      {GENERATION_LABEL}: \"{}\"", deployment.generation),
        ];
        for route in routes {
            for (key, value) in &route.labels {
                labels.push(format!(
                    "      {}: \"{}\"",
                    yaml_quote(key),
                    yaml_quote(value)
                ));
            }
        }
        let content = format!(
            "services:\n  {exposed_service}:\n    networks:\n      - {PROXY_NETWORK}\n    labels:\n{}networks:\n  {PROXY_NETWORK}:\n    external: true\n",
            labels.join("\n"),
        );
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
            self.write_override(&stage, deployment, &[]).await?;
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
        let records = parse_ps(&output);
        let statuses: Vec<String> = records.iter().map(status).collect();
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
            "--no-color".to_owned(),
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
            self.write_override(&stage, deployment, &routes).await?;
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

/// Performs host-independent Compose validation before desired configuration is persisted.
pub fn validate_submission_yaml(source: &str) -> std::result::Result<(), &'static str> {
    preflight_yaml(source).map_err(|error| match error {
        Error::Policy(message) => message,
        _ => "invalid YAML",
    })
}

fn preflight_yaml(source: &str) -> Result<()> {
    if source.len() > MAX_YAML_BYTES {
        return Err(Error::Policy("YAML exceeds 1 MiB"));
    }
    reject_yaml_mechanisms(source)?;
    let documents = YamlLoader::load_from_str(source).map_err(|_| Error::Policy("invalid YAML"))?;
    if documents.len() != 1 {
        return Err(Error::Policy("exactly one YAML document required"));
    }
    let mut services = 0;
    inspect_yaml(&documents[0], 0, &mut services)?;
    if services == 0 {
        return Err(Error::Policy("Compose services are required"));
    }
    Ok(())
}

fn reject_yaml_mechanisms(source: &str) -> Result<()> {
    enum Context {
        Mapping {
            keys: HashSet<String>,
            expecting_key: bool,
        },
        Sequence,
    }

    struct Sink {
        contexts: Vec<Context>,
        rejected: bool,
    }

    impl EventReceiver for Sink {
        fn on_event(&mut self, event: Event) {
            match event {
                Event::Alias(_) => self.rejected = true,
                Event::Scalar(value, _, anchor, tag) => {
                    if anchor != 0 || tag.is_some() {
                        self.rejected = true;
                    }
                    if let Some(Context::Mapping {
                        keys,
                        expecting_key,
                    }) = self.contexts.last_mut()
                    {
                        if *expecting_key {
                            if !keys.insert(value) {
                                self.rejected = true;
                            }
                            *expecting_key = false;
                        } else {
                            *expecting_key = true;
                        }
                    }
                }
                Event::MappingStart(anchor, tag) => {
                    if anchor != 0 || tag.is_some() {
                        self.rejected = true;
                    }
                    mark_value_complete(&mut self.contexts);
                    self.contexts.push(Context::Mapping {
                        keys: HashSet::new(),
                        expecting_key: true,
                    });
                }
                Event::SequenceStart(anchor, tag) => {
                    if anchor != 0 || tag.is_some() {
                        self.rejected = true;
                    }
                    mark_value_complete(&mut self.contexts);
                    self.contexts.push(Context::Sequence);
                }
                Event::MappingEnd | Event::SequenceEnd => {
                    self.contexts.pop();
                    mark_value_complete(&mut self.contexts);
                }
                Event::Nothing
                | Event::StreamStart
                | Event::StreamEnd
                | Event::DocumentStart
                | Event::DocumentEnd => {}
            }
        }
    }

    fn mark_value_complete(contexts: &mut [Context]) {
        if let Some(Context::Mapping { expecting_key, .. }) = contexts.last_mut() {
            *expecting_key = true;
        }
    }

    let mut sink = Sink {
        contexts: Vec::new(),
        rejected: false,
    };
    Parser::new_from_str(source)
        .load(&mut sink, true)
        .map_err(|_| Error::Policy("invalid YAML"))?;
    if sink.rejected {
        Err(Error::Policy(
            "YAML aliases, anchors, tags, and duplicate keys are forbidden",
        ))
    } else {
        Ok(())
    }
}

fn inspect_yaml(value: &Yaml, depth: usize, services: &mut usize) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(Error::Policy("YAML nesting exceeds 64 levels"));
    }
    match value {
        Yaml::Alias(_) => return Err(Error::Policy("YAML aliases are forbidden")),
        Yaml::Hash(map) => {
            let mut keys = BTreeSet::new();
            for (key, child) in map {
                let key = match key {
                    Yaml::String(key) => key,
                    _ => return Err(Error::Policy("mapping keys must be strings")),
                };
                if !keys.insert(key) {
                    return Err(Error::Policy("duplicate YAML key"));
                }
                if key == "services"
                    && let Yaml::Hash(service_map) = child
                {
                    *services = service_map.len();
                    if *services > MAX_SERVICES {
                        return Err(Error::Policy("too many services"));
                    }
                }
                if matches!(
                    key.as_str(),
                    "build"
                        | "ports"
                        | "network_mode"
                        | "pid"
                        | "ipc"
                        | "uts"
                        | "privileged"
                        | "cap_add"
                        | "cap_drop"
                        | "devices"
                        | "gpus"
                        | "volumes_from"
                        | "runtime"
                        | "security_opt"
                        | "sysctls"
                        | "include"
                        | "extends"
                        | "profiles"
                        | "env_file"
                        | "label_file"
                        | "external"
                        | "driver"
                        | "driver_opts"
                ) {
                    return Err(Error::Policy("unsupported or unsafe Compose field"));
                }
                if key == "image" && !matches!(child, Yaml::String(image) if is_digest_image(image))
                {
                    return Err(Error::Policy("every Compose image must use digest"));
                }
                if key == "volumes"
                    && let Yaml::Hash(volumes) = child
                    && volumes.values().any(|volume| {
                        matches!(volume, Yaml::Hash(options) if options.keys().any(|option| {
                            matches!(option, Yaml::String(name) if matches!(name.as_str(), "external" | "name" | "driver" | "driver_opts"))
                        }))
                    })
                {
                    return Err(Error::Policy(
                        "external, named, or configured volumes are forbidden",
                    ));
                }
                if key == "volumes"
                    && let Yaml::Array(volumes) = child
                    && volumes.iter().any(|volume| {
                        matches!(volume, Yaml::String(value) if value.split_once(':').map_or(value.as_str(), |(source, _)| source).contains('/') || value.split_once(':').map_or(value.as_str(), |(source, _)| source).starts_with('.'))
                    })
                {
                    return Err(Error::Policy("bind mounts are forbidden"));
                }
                inspect_yaml(child, depth + 1, services)?;
            }
        }
        Yaml::Array(items) => {
            for child in items {
                inspect_yaml(child, depth + 1, services)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_canonical(value: &Value, generated: bool) -> Result<()> {
    let services = value
        .get("services")
        .and_then(Value::as_object)
        .ok_or(Error::Policy("services must be an object"))?;
    if services.is_empty() || services.len() > MAX_SERVICES {
        return Err(Error::Policy("invalid service count"));
    }
    validate_json_keys(value, generated, &[])?;
    if let Some(volumes) = value.get("volumes").and_then(Value::as_object) {
        for volume in volumes.values() {
            let Some(volume) = volume.as_object() else {
                continue;
            };
            if volume.get("external").and_then(Value::as_bool) == Some(true)
                || volume.contains_key("name")
                || volume.contains_key("driver")
                || volume.contains_key("driver_opts")
            {
                return Err(Error::Policy(
                    "only unnamed local Compose volumes are allowed",
                ));
            }
        }
    }
    for service in services.values() {
        let service = service
            .as_object()
            .ok_or(Error::Policy("service must be an object"))?;
        const ALLOWED_SERVICE_KEYS: &[&str] = &[
            "image",
            "command",
            "entrypoint",
            "environment",
            "volumes",
            "networks",
            "depends_on",
            "restart",
            "healthcheck",
            "working_dir",
            "user",
            "stdin_open",
            "tty",
            "read_only",
            "init",
            "stop_signal",
            "stop_grace_period",
            "expose",
            "platform",
        ];
        if service.keys().any(|key| {
            !(ALLOWED_SERVICE_KEYS.contains(&key.as_str()) || generated && key == "labels")
        }) {
            return Err(Error::Policy("unsupported Compose service field"));
        }
        if let Some(volumes) = service.get("volumes").and_then(Value::as_array) {
            for volume in volumes {
                match volume {
                    Value::String(value) => {
                        let source = value
                            .split_once(':')
                            .map_or(value.as_str(), |(source, _)| source);
                        if source.contains('/') || source.starts_with('.') || source.is_empty() {
                            return Err(Error::Policy("bind mounts are forbidden"));
                        }
                    }
                    Value::Object(value) => {
                        if value.get("type").and_then(Value::as_str) != Some("volume")
                            || value.get("source").and_then(Value::as_str).is_none()
                            || value.contains_key("volume")
                        {
                            return Err(Error::Policy("only named local volumes are allowed"));
                        }
                    }
                    _ => return Err(Error::Policy("invalid Compose volume")),
                }
            }
        }
        let image = service
            .get("image")
            .and_then(Value::as_str)
            .ok_or(Error::Policy("every service needs image"))?;
        if !is_digest_image(image) {
            return Err(Error::Policy("every Compose image must use digest"));
        }
    }
    Ok(())
}

fn validate_json_keys(value: &Value, generated: bool, path: &[&str]) -> Result<()> {
    if let Value::Object(map) = value {
        for (key, child) in map {
            let generated_network =
                generated && path == ["networks", "ignitify-proxy"] && key == "external";
            let forbidden = matches!(
                key.as_str(),
                "build"
                    | "ports"
                    | "network_mode"
                    | "pid"
                    | "ipc"
                    | "uts"
                    | "privileged"
                    | "cap_add"
                    | "cap_drop"
                    | "devices"
                    | "gpus"
                    | "volumes_from"
                    | "runtime"
                    | "security_opt"
                    | "sysctls"
                    | "external_links"
                    | "configs"
                    | "secrets"
                    | "driver_opts"
                    | "<<"
            ) || (key.starts_with("traefik.") && !generated)
                || (key == "external" && child.as_bool() == Some(true) && !generated_network)
                || key == "bind";
            if key == "type" && child.as_str() == Some("bind") {
                return Err(Error::Policy("bind mounts are forbidden"));
            }
            if forbidden {
                return Err(Error::Policy("unsupported or unsafe Compose field"));
            }
            let next_path = if path.len() < 2 {
                let mut next_path = path.to_vec();
                next_path.push(key.as_str());
                next_path
            } else {
                path.to_vec()
            };
            validate_json_keys(child, generated, &next_path)?;
        }
    } else if let Value::Array(items) = value {
        for child in items {
            validate_json_keys(child, generated, path)?;
        }
    }
    Ok(())
}

fn ensure_exposed_service(value: &Value, service: &str) -> Result<()> {
    if value
        .get("services")
        .and_then(Value::as_object)
        .is_some_and(|services| services.contains_key(service))
    {
        Ok(())
    } else {
        Err(Error::Policy("selected exposed service does not exist"))
    }
}

fn is_digest_image(value: &str) -> bool {
    ignitify_domain::is_digest_image_reference(value)
}

fn yaml_quote(value: &str) -> String {
    value.replace('"', "\\\"")
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Docker executable path must be absolute")]
    InvalidDockerPath,
    #[error("Compose specification is not supported by this runtime")]
    UnsupportedSpec,
    #[error("Compose policy rejected input: {0}")]
    Policy(&'static str),
    #[error("Docker Compose command failed: {0}")]
    CommandFailed(String),
    #[error("Docker Compose returned invalid canonical configuration")]
    InvalidCanonicalConfig,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::{MAX_DEPTH, preflight_yaml, validate_canonical};
    use serde_json::json;

    #[cfg(unix)]
    use {
        ignitify_control_plane::{ImageRuntime, RuntimeDeployment},
        ignitify_domain::{DeploymentId, ServiceId, ServiceSpec},
        std::{fs, os::unix::fs::PermissionsExt},
    };

    #[cfg(unix)]
    fn deployment() -> RuntimeDeployment {
        RuntimeDeployment {
            id: DeploymentId::new("00000000-0000-0000-0000-000000000001").unwrap(),
            service_id: ServiceId::new("00000000-0000-0000-0000-000000000002").unwrap(),
            generation: 1,
            spec: ServiceSpec::compose(
                "services:\n  web:\n    image: nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
                "web",
                Some(8080),
            )
            .unwrap(),
        }
    }

    #[cfg(unix)]
    fn fake_docker(temp: &tempfile::TempDir) -> std::path::PathBuf {
        let executable = temp.path().join("fake-docker");
        fs::write(
            &executable,
            "#!/bin/sh\nprintf 'cwd=<%s> ' \"$PWD\" >> \"$0.log\"\nprintf 'args=' >> \"$0.log\"\nfor argument in \"$@\"; do printf '<%s>' \"$argument\" >> \"$0.log\"; done\nprintf '\\n' >> \"$0.log\"\nenv | sort > \"$0.env\"\nfor argument in \"$@\"; do\n  if [ \"$argument\" = logs ]; then\n    printf 'stdout log\\n'\n    printf 'stderr log\\n' >&2\n    exit 0\n  fi\n  if [ \"$argument\" = up ] && [ -f \"$0.fail-up\" ]; then\n    printf 'up failed\\n' >&2\n    exit 1\n  fi\ndone\nfor argument in \"$@\"; do\n  if [ \"$argument\" = config ]; then\n    printf '{\"services\":{\"web\":{\"image\":\"nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}}'\n    exit 0\n  fi\ndone\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        executable
    }

    #[test]
    fn compose_command_uses_fixed_argument_order() {
        let args = super::compose_args(
            std::path::Path::new("/var/lib/ignitify/compose/service/1"),
            "ignitify-service-g1",
        );
        let stage = std::path::Path::new("/var/lib/ignitify/compose/service/1");
        assert_eq!(
            args,
            [
                "compose".to_owned(),
                "--project-directory".to_owned(),
                stage.display().to_string(),
                "--project-name".to_owned(),
                "ignitify-service-g1".to_owned(),
                "--file".to_owned(),
                stage.join("compose.yaml").display().to_string(),
                "--env-file".to_owned(),
                stage.join("ignitify.env").display().to_string(),
            ]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn fake_docker_enforces_fixed_commands_and_cleans_failed_stages() {
        let temp = tempfile::tempdir().unwrap();
        let docker = fake_docker(&temp);
        let root = temp.path().join("stages");
        let runtime = super::ComposeRuntime::new(&docker, &root).unwrap();
        let deployment = deployment();
        let stage = root.join(deployment.service_id.as_str()).join("1");
        let project = "ignitify-00000000-0000-0000-0000-000000000002-g1";

        let runtime_ref = runtime
            .start(&deployment, vec!["TOKEN=value".to_owned()])
            .await
            .unwrap();
        assert_eq!(runtime_ref, project);
        assert_eq!(
            fs::read_to_string(stage.join("ignitify.env")).unwrap(),
            "TOKEN=value"
        );
        assert_eq!(
            fs::read_to_string(docker.with_extension("env")).unwrap(),
            "PATH=/usr/bin:/bin\n"
        );

        let calls = fs::read_to_string(docker.with_extension("log")).unwrap();
        let base = format!(
            "cwd=<{stage}> args=<compose><--project-directory><{stage}><--project-name><{project}><--file><{stage}/compose.yaml><--env-file><{stage}/ignitify.env>",
            stage = stage.display(),
        );
        assert_eq!(
            calls.lines().collect::<Vec<_>>(),
            [
                format!("{base}<config><--format><json>"),
                format!(
                    "{base}<--file><{}/ignitify.override.yaml><config><--format><json>",
                    stage.display()
                ),
                format!(
                    "{base}<--file><{}/ignitify.override.yaml><up><--detach><--no-build><--remove-orphans>",
                    stage.display()
                ),
            ],
        );

        let logs = runtime.logs(project, 0).await.unwrap();
        assert_eq!(
            logs.into_iter()
                .map(|log| (log.stream, log.line))
                .collect::<Vec<_>>(),
            [
                ("stdout".to_owned(), "stdout log".to_owned()),
                ("stderr".to_owned(), "stderr log".to_owned())
            ],
        );

        let failed_deployment = RuntimeDeployment {
            generation: 2,
            ..deployment
        };
        let failed_stage = root
            .join(failed_deployment.service_id.as_str())
            .join(failed_deployment.generation.to_string());
        fs::write(docker.with_extension("fail-up"), "").unwrap();
        assert!(runtime.start(&failed_deployment, vec![]).await.is_err());
        assert!(!failed_stage.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn deploys_private_compose_service_with_generated_labels_when_opted_in() {
        if std::env::var("IGNITIFY_DOCKER_TEST").as_deref() != Ok("1") {
            return;
        }
        let docker = std::path::PathBuf::from("/usr/bin/docker");
        if !docker.exists() {
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let runtime = super::ComposeRuntime::new(&docker, temp.path()).unwrap();
        let service_id = ServiceId::new(uuid::Uuid::new_v4().to_string()).unwrap();
        let deployment = RuntimeDeployment {
            id: DeploymentId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
            service_id,
            generation: 1,
            spec: ServiceSpec::compose(
                "services:\n  web:\n    image: caddy:2.11.4-alpine@sha256:98eb57d882ccd5213d1688764db10c1ca2c58a1ca3a6717a3411ad798f7a423a\n",
                "web",
                Some(80),
            )
            .unwrap(),
        };
        let network = super::PROXY_NETWORK;
        let created_network = !tokio::process::Command::new(&docker)
            .args(["network", "inspect", network])
            .output()
            .await
            .unwrap()
            .status
            .success();
        if created_network {
            let output = tokio::process::Command::new(&docker)
                .args(["network", "create", network])
                .output()
                .await
                .unwrap();
            assert!(output.status.success());
        }
        let runtime_ref = runtime.start(&deployment, vec![]).await.unwrap();
        let labels = std::collections::BTreeMap::from([
            ("traefik.enable".to_owned(), "true".to_owned()),
            (
                "traefik.http.routers.ignitify-test.rule".to_owned(),
                "Host(`compose-test.example.com`)".to_owned(),
            ),
            (
                "traefik.http.services.ignitify-test.loadbalancer.server.port".to_owned(),
                "80".to_owned(),
            ),
        ]);
        let result = async {
            runtime
                .reconcile_routes(
                    &deployment,
                    &runtime_ref,
                    vec![],
                    vec![ignitify_control_plane::IngressRoute {
                        labels,
                        network: network.to_owned(),
                    }],
                )
                .await
                .map_err(|error| error.to_string())?;
            let inspect = tokio::process::Command::new(&docker)
                .args(["inspect", &runtime_ref])
                .output()
                .await
                .map_err(|error| error.to_string())?;
            if !inspect.status.success() {
                return Err("could not inspect Compose container".to_owned());
            }
            let inspect = String::from_utf8_lossy(&inspect.stdout);
            if !inspect.contains("com.ignitify.managed")
                || !inspect.contains("traefik.enable")
                || !inspect.contains("ignitify-proxy")
                || inspect.contains("\"PortBindings\": {")
            {
                return Err(
                    "Compose runtime did not preserve private managed ingress contract".to_owned(),
                );
            }
            Ok(())
        }
        .await;
        let stop = runtime
            .stop(
                &runtime_ref,
                deployment.service_id.as_str(),
                deployment.generation,
            )
            .await;
        if created_network {
            let _ = tokio::process::Command::new(&docker)
                .args(["network", "rm", network])
                .output()
                .await;
        }
        assert!(stop.unwrap());
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }

    #[test]
    fn compose_policy_error_stays_typed() {
        assert!(matches!(
            super::control_error(super::Error::Policy("invalid YAML")),
            ignitify_control_plane::Error::Policy("invalid YAML")
        ));
    }

    #[test]
    fn accepts_safe_digest_compose() {
        let yaml = "services:\n  web:\n    image: nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n    volumes:\n      - data:/data\nvolumes:\n  data: {}\n";
        preflight_yaml(yaml).unwrap();
        validate_canonical(
            &json!({"services":{"web":{"image":"nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}}),
            false,
        )
        .unwrap();
    }

    #[test]
    fn generated_labels_are_allowed_only_after_platform_override() {
        let value = json!({
            "services": {
                "web": {
                    "image": "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "labels": { "traefik.enable": "true" }
                }
            }
        });

        assert!(validate_canonical(&value, false).is_err());
        assert!(validate_canonical(&value, true).is_ok());
    }

    #[test]
    fn rejects_host_escape_fields() {
        for key in [
            "build",
            "ports",
            "privileged",
            "devices",
            "network_mode",
            "pid",
            "ipc",
            "uts",
            "cap_add",
            "gpus",
            "volumes_from",
        ] {
            let value = json!({"services":{"web":{"image":"nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", key: []}}});
            assert!(validate_canonical(&value, false).is_err(), "{key}");
        }
        assert!(validate_canonical(&json!({"services":{"web":{"image":"nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "labels":{"traefik.enable":"true"}}}}), false).is_err());
        assert!(validate_canonical(&json!({"services":{"web":{"image":"nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumes":["/tmp:/data"]}}}), false).is_err());
        assert!(validate_canonical(&json!({"services":{"web":{"image":"nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}, "volumes":{"data":{"external":true}}}), false).is_err());
    }

    #[test]
    fn fixture_safe_documents_pass_preflight() {
        for fixture in [
            include_str!("../tests/fixtures/safe-web.yaml"),
            include_str!("../tests/fixtures/safe-volume.yaml"),
        ] {
            preflight_yaml(fixture).unwrap();
        }
    }

    #[test]
    fn fixture_forbidden_yaml_document_fails_preflight() {
        assert!(
            preflight_yaml(include_str!(
                "../tests/fixtures/rejected-yaml-mechanisms.yaml"
            ))
            .is_err()
        );
    }

    #[test]
    fn rejects_aliases_and_deep_documents() {
        assert!(
            preflight_yaml("services:\n  web: &web\n    image: nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n").is_err()
        );
        let mut value = String::from("x:");
        for _ in 0..=MAX_DEPTH {
            value.push_str("\n  x:");
        }
        value.push_str(" true\n");
        assert!(preflight_yaml(&value).is_err());
    }
}
