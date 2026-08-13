//! Isolated Git checkout and image build adapter for application sources.

mod build_support;
mod checkout;
mod command_failure;
mod github_app;
mod sensitive_file;
mod source_spec;

use std::{
    env,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ignitify_control_plane::{
    AgeCipher, DeploymentLogSink, Error as ControlError, SourceBuild, SourceBuildOutput,
};
use ignitify_db::{Database, RemoteBuilderConnection};
use ignitify_domain::{ApplicationBuilder, ServiceSourceConfig, is_digest_image_reference};
use serde::Deserialize;
use tokio::{
    fs,
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

use build_support::{BuildError, BuildLimiter, source_build_error};
use sensitive_file::write_sensitive_file;
use source_spec::{is_local_image_id, relative_path, static_dockerfile};

const DEFAULT_BUILD_ROOT: &str = "data/builds";
const DEFAULT_COMMAND_TIMEOUT_SECONDS: u64 = 900;
const DEFAULT_STATIC_BUILD_IMAGE: &str = "node:22.23.1-alpine3.24@sha256:16e22a550f3863206a3f701448c45f7912c6896a62de43add43bb9c86130c3e2";
const DEFAULT_CADDY_IMAGE: &str =
    "caddy:2.11.4-alpine@sha256:98eb57d882ccd5213d1688764db10c1ca2c58a1ca3a6717a3411ad798f7a423a";
const DEFAULT_RAILPACK_FRONTEND_IMAGE: &str = "ghcr.io/railwayapp/railpack-frontend:latest@sha256:bc73534934e7929ab3dc41765fb7e25c8c69d9be98c43ef8792fea51f65317bd";

#[derive(Clone)]
pub struct GitSourceBuild {
    database: Database,
    cipher: Arc<AgeCipher>,
    root: PathBuf,
    git_bin: String,
    docker_bin: String,
    railpack_bin: String,
    railpack_frontend_image: String,
    static_build_image: String,
    static_runtime_image: String,
    allow_local_builds: bool,
    command_timeout: std::time::Duration,
    build_limiter: Arc<BuildLimiter>,
}

impl GitSourceBuild {
    pub fn from_environment(
        database: Database,
        identity: impl AsRef<str>,
    ) -> Result<Self, ControlError> {
        Ok(Self {
            database,
            cipher: Arc::new(AgeCipher::from_identity(identity)?),
            root: env_value("IGNITIFY_SOURCE_BUILD_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(DEFAULT_BUILD_ROOT)),
            git_bin: env_value("IGNITIFY_GIT_BIN").unwrap_or_else(|| "git".to_owned()),
            docker_bin: env_value("IGNITIFY_DOCKER_BIN").unwrap_or_else(|| "docker".to_owned()),
            railpack_bin: env_value("IGNITIFY_RAILPACK_BIN").unwrap_or_else(default_railpack_bin),
            railpack_frontend_image: env_value("IGNITIFY_RAILPACK_FRONTEND_IMAGE")
                .unwrap_or_else(|| DEFAULT_RAILPACK_FRONTEND_IMAGE.to_owned()),
            static_build_image: env_value("IGNITIFY_STATIC_BUILD_IMAGE")
                .unwrap_or_else(|| DEFAULT_STATIC_BUILD_IMAGE.to_owned()),
            static_runtime_image: env_value("IGNITIFY_STATIC_RUNTIME_IMAGE")
                .unwrap_or_else(|| DEFAULT_CADDY_IMAGE.to_owned()),
            allow_local_builds: env_value("IGNITIFY_ALLOW_LOCAL_BUILDS")
                .is_some_and(|value| value == "true"),
            command_timeout: command_timeout(),
            build_limiter: Arc::new(BuildLimiter::default()),
        })
    }

    async fn build_inner(
        &self,
        deployment: &ignitify_db::DeploymentRecord,
        logs: &DeploymentLogSink,
    ) -> Result<Option<SourceBuildOutput>, BuildError> {
        let Some(source) = deployment.source_config.as_ref() else {
            return Ok(None);
        };
        match source.source.as_str() {
            "application" => self.build_application(deployment, source, logs).await,
            "compose" if source.provider_id.is_some() => {
                self.build_compose(deployment, source, logs).await
            }
            _ => Ok(None),
        }
    }

    async fn build_application(
        &self,
        deployment: &ignitify_db::DeploymentRecord,
        source: &ServiceSourceConfig,
        logs: &DeploymentLogSink,
    ) -> Result<Option<SourceBuildOutput>, BuildError> {
        let builder = source.builder.ok_or(BuildError::InvalidSource)?;
        if builder == ApplicationBuilder::Spa {
            return Err(BuildError::UnsupportedBuilder);
        }
        if deployment.deployment_destination_id.is_some()
            && self.database.remote_builders().active().await?.is_none()
        {
            return Err(BuildError::RemoteBuilderRequired);
        }
        if builder == ApplicationBuilder::Static && deployment.spec.internal_port() != Some(80) {
            return Err(BuildError::StaticPort);
        }
        logs.system("Checking out Git source").await?;
        let checkout = self.checkout_source(deployment, source).await?;
        logs.system(format!("Checked out revision {}", checkout.revision))
            .await?;
        let result = self
            .build_image(
                deployment.id.as_str(),
                builder,
                source,
                &checkout.path,
                logs,
            )
            .await;
        cleanup_checkout(&checkout).await;
        let local_image_id = result?;
        Ok(Some(SourceBuildOutput {
            source_revision: checkout.revision,
            local_image_id: Some(local_image_id),
            runtime_spec: None,
        }))
    }

    async fn build_compose(
        &self,
        deployment: &ignitify_db::DeploymentRecord,
        source: &ServiceSourceConfig,
        logs: &DeploymentLogSink,
    ) -> Result<Option<SourceBuildOutput>, BuildError> {
        logs.system("Checking out Git Compose source").await?;
        let checkout = self.checkout_source(deployment, source).await?;
        let result = self.compose_spec(deployment, source, &checkout.path).await;
        cleanup_checkout(&checkout).await;
        Ok(Some(SourceBuildOutput {
            source_revision: checkout.revision,
            local_image_id: None,
            runtime_spec: Some(result?),
        }))
    }

    async fn compose_spec(
        &self,
        deployment: &ignitify_db::DeploymentRecord,
        source: &ServiceSourceConfig,
        checkout: &Path,
    ) -> Result<ignitify_domain::ServiceSpec, BuildError> {
        let compose_path = relative_path(
            source
                .dockerfile_path
                .as_deref()
                .unwrap_or("docker-compose.yml"),
        )?;
        let yaml = fs::read_to_string(checkout.join(compose_path)).await?;
        source_spec::compose_runtime_spec(&deployment.spec, yaml)
    }

    async fn build_image(
        &self,
        deployment_id: &str,
        builder: ApplicationBuilder,
        source: &ServiceSourceConfig,
        checkout: &Path,
        logs: &DeploymentLogSink,
    ) -> Result<String, BuildError> {
        if let Some(remote) = self.database.remote_builders().active().await? {
            return self
                .build_remote_image(deployment_id, builder, source, checkout, &remote, logs)
                .await;
        }
        if !self.allow_local_builds {
            return Err(BuildError::LocalBuilderDisabled);
        }
        let tag = format!("ignitify-build:{deployment_id}");
        match builder {
            ApplicationBuilder::Dockerfile => {
                let dockerfile =
                    relative_path(source.dockerfile_path.as_deref().unwrap_or("Dockerfile"))?;
                self.docker_build(&tag, checkout, checkout.join(dockerfile), logs)
                    .await?;
            }
            ApplicationBuilder::Railpack => {
                let frontend = configured_digest_image(
                    "IGNITIFY_RAILPACK_FRONTEND_IMAGE",
                    &self.railpack_frontend_image,
                )?;
                let plan = checkout.join(".ignitify-railpack-plan.json");
                let mut prepare = Command::new(&self.railpack_bin);
                prepare
                    .arg("prepare")
                    .arg(checkout)
                    .arg("--plan-out")
                    .arg(&plan);
                if let Some(command) = source.build_command.as_deref() {
                    prepare.arg("--build-cmd").arg(command);
                }
                self.run_logged(&mut prepare, "railpack prepare", logs)
                    .await?;
                let mut build = Command::new(&self.docker_bin);
                build
                    .args([
                        "buildx",
                        "build",
                        "--load",
                        "--progress=plain",
                        "--tag",
                        &tag,
                    ])
                    .arg("--build-arg")
                    .arg(format!("BUILDKIT_SYNTAX={frontend}"))
                    .arg("--file")
                    .arg(plan)
                    .arg(checkout);
                self.run_logged(&mut build, "railpack image build", logs)
                    .await?;
            }
            ApplicationBuilder::Static => {
                let build_image = configured_digest_image(
                    "IGNITIFY_STATIC_BUILD_IMAGE",
                    &self.static_build_image,
                )?;
                if !is_digest_image_reference(&self.static_runtime_image) {
                    return Err(BuildError::InvalidStaticRuntimeImage);
                }
                let output = relative_path(source.output_directory.as_deref().unwrap_or("dist"))?;
                let command = source
                    .build_command
                    .as_deref()
                    .unwrap_or("npm ci && npm run build");
                let dockerfile = checkout.join(".ignitify-static.Dockerfile");
                fs::write(
                    &dockerfile,
                    static_dockerfile(&build_image, &self.static_runtime_image, command, &output),
                )
                .await?;
                self.docker_build(&tag, checkout, dockerfile, logs).await?;
            }
            ApplicationBuilder::Spa => return Err(BuildError::UnsupportedBuilder),
        }
        self.local_image_id(&tag).await
    }

    async fn docker_build(
        &self,
        tag: &str,
        checkout: &Path,
        dockerfile: PathBuf,
        logs: &DeploymentLogSink,
    ) -> Result<(), BuildError> {
        let mut command = Command::new(&self.docker_bin);
        command
            .args([
                "buildx",
                "build",
                "--load",
                "--progress=plain",
                "--tag",
                tag,
                "--file",
            ])
            .arg(dockerfile)
            .arg(checkout);
        self.run_logged(&mut command, "docker image build", logs)
            .await
    }

    async fn build_remote_image(
        &self,
        deployment_id: &str,
        builder: ApplicationBuilder,
        source: &ServiceSourceConfig,
        checkout: &Path,
        remote: &RemoteBuilderConnection,
        logs: &DeploymentLogSink,
    ) -> Result<String, BuildError> {
        logs.system("Connecting to remote builder").await?;
        let session = self
            .open_remote_builder(deployment_id, remote, logs)
            .await?;
        let image = format!("{}:ignitify-{}", remote.registry_repository, deployment_id);
        let metadata = checkout.join(".ignitify-build-metadata.json");
        let result = self
            .build_remote_image_with_session(
                RemoteBuildContext {
                    session: &session,
                    image: &image,
                    metadata: &metadata,
                    checkout,
                    logs,
                },
                builder,
                source,
            )
            .await;
        self.close_remote_builder(session).await;
        result
    }

    async fn build_remote_image_with_session(
        &self,
        context: RemoteBuildContext<'_>,
        builder: ApplicationBuilder,
        source: &ServiceSourceConfig,
    ) -> Result<String, BuildError> {
        match builder {
            ApplicationBuilder::Dockerfile => {
                let dockerfile =
                    relative_path(source.dockerfile_path.as_deref().unwrap_or("Dockerfile"))?;
                self.remote_docker_build(
                    context.session,
                    context.image,
                    context.metadata,
                    context.checkout,
                    context.checkout.join(dockerfile),
                    context.logs,
                )
                .await?;
            }
            ApplicationBuilder::Railpack => {
                let frontend = configured_digest_image(
                    "IGNITIFY_RAILPACK_FRONTEND_IMAGE",
                    &self.railpack_frontend_image,
                )?;
                let plan = context.checkout.join(".ignitify-railpack-plan.json");
                let mut prepare = Command::new(&self.railpack_bin);
                prepare
                    .arg("prepare")
                    .arg(context.checkout)
                    .arg("--plan-out")
                    .arg(&plan);
                if let Some(command) = source.build_command.as_deref() {
                    prepare.arg("--build-cmd").arg(command);
                }
                self.run_logged(&mut prepare, "railpack prepare", context.logs)
                    .await?;
                let mut build = Command::new(&self.docker_bin);
                build
                    .args([
                        "buildx",
                        "build",
                        "--builder",
                        &context.session.name,
                        "--push",
                        "--progress=plain",
                        "--tag",
                        context.image,
                        "--metadata-file",
                    ])
                    .arg(context.metadata)
                    .arg("--build-arg")
                    .arg(format!("BUILDKIT_SYNTAX={frontend}"))
                    .arg("--file")
                    .arg(plan)
                    .arg(context.checkout);
                self.run_logged(&mut build, "remote railpack image build", context.logs)
                    .await?;
            }
            ApplicationBuilder::Static => {
                let build_image = configured_digest_image(
                    "IGNITIFY_STATIC_BUILD_IMAGE",
                    &self.static_build_image,
                )?;
                if !is_digest_image_reference(&self.static_runtime_image) {
                    return Err(BuildError::InvalidStaticRuntimeImage);
                }
                let output = relative_path(source.output_directory.as_deref().unwrap_or("dist"))?;
                let command = source
                    .build_command
                    .as_deref()
                    .unwrap_or("npm ci && npm run build");
                let dockerfile = context.checkout.join(".ignitify-static.Dockerfile");
                fs::write(
                    &dockerfile,
                    static_dockerfile(&build_image, &self.static_runtime_image, command, &output),
                )
                .await?;
                self.remote_docker_build(
                    context.session,
                    context.image,
                    context.metadata,
                    context.checkout,
                    dockerfile,
                    context.logs,
                )
                .await?;
            }
            ApplicationBuilder::Spa => return Err(BuildError::UnsupportedBuilder),
        }
        remote_image_reference(context.image, context.metadata).await
    }

    async fn remote_docker_build(
        &self,
        session: &RemoteBuildSession,
        image: &str,
        metadata: &Path,
        checkout: &Path,
        dockerfile: PathBuf,
        logs: &DeploymentLogSink,
    ) -> Result<(), BuildError> {
        let mut command = Command::new(&self.docker_bin);
        command
            .args([
                "buildx",
                "build",
                "--builder",
                &session.name,
                "--push",
                "--progress=plain",
                "--tag",
                image,
                "--metadata-file",
            ])
            .arg(metadata)
            .arg("--file")
            .arg(dockerfile)
            .arg(checkout);
        self.run_logged(&mut command, "remote Docker image build", logs)
            .await
    }

    async fn open_remote_builder(
        &self,
        deployment_id: &str,
        remote: &RemoteBuilderConnection,
        logs: &DeploymentLogSink,
    ) -> Result<RemoteBuildSession, BuildError> {
        let certificate_dir = self.root.join(format!("{deployment_id}.remote-builder"));
        remove_dir_if_exists(&certificate_dir).await?;
        fs::create_dir_all(&certificate_dir).await?;
        set_sensitive_directory_permissions(&certificate_dir).await?;
        let ca_path = certificate_dir.join("ca.pem");
        let certificate_path = certificate_dir.join("client.pem");
        let key_path = certificate_dir.join("client-key.pem");
        let result = async {
            write_sensitive_file(
                &ca_path,
                &self.cipher.decrypt(&remote.ca_certificate_ciphertext)?,
            )
            .await?;
            write_sensitive_file(
                &certificate_path,
                &self.cipher.decrypt(&remote.client_certificate_ciphertext)?,
            )
            .await?;
            write_sensitive_file(
                &key_path,
                &self.cipher.decrypt(&remote.client_key_ciphertext)?,
            )
            .await?;
            let name = format!("ignitify-remote-{deployment_id}");
            let mut create = Command::new(&self.docker_bin);
            let mut options = format!(
                "cacert={},cert={},key={}",
                ca_path.display(),
                certificate_path.display(),
                key_path.display()
            );
            if let Some(server_name) = remote.tls_server_name.as_deref() {
                options.push_str(",servername=");
                options.push_str(server_name);
            }
            create
                .args(["buildx", "create", "--name", &name, "--driver", "remote"])
                .arg("--driver-opt")
                .arg(options)
                .arg(&remote.endpoint);
            self.run_logged(&mut create, "remote builder connection", logs)
                .await?;
            Ok(RemoteBuildSession {
                name,
                certificate_dir: certificate_dir.clone(),
            })
        }
        .await;
        if result.is_err() {
            let _ = remove_dir_if_exists(&certificate_dir).await;
        }
        result
    }

    async fn close_remote_builder(&self, session: RemoteBuildSession) {
        let mut remove = Command::new(&self.docker_bin);
        remove.args(["buildx", "rm", "--force", &session.name]);
        let _ = self.run(&mut remove, "remote builder cleanup").await;
        let _ = remove_dir_if_exists(&session.certificate_dir).await;
    }

    async fn local_image_id(&self, tag: &str) -> Result<String, BuildError> {
        let image_id = self
            .output(
                Command::new(&self.docker_bin)
                    .args(["image", "inspect", "--format", "{{.Id}}", tag]),
                "docker image inspect",
            )
            .await?;
        if !is_local_image_id(&image_id) {
            return Err(BuildError::InvalidImageId);
        }
        Ok(image_id)
    }

    async fn run(&self, command: &mut Command, action: &'static str) -> Result<(), BuildError> {
        if command_failure::is_git_action(action) {
            let output = self
                .wait_for_command(
                    command
                        .kill_on_drop(true)
                        .stdout(Stdio::null())
                        .stderr(Stdio::piped())
                        .output(),
                    action,
                )
                .await?;
            return if output.status.success() {
                Ok(())
            } else {
                Err(BuildError::GitCheckout(
                    command_failure::classify_git_failure(action, &output.stderr),
                ))
            };
        }
        let status = self
            .wait_for_command(
                command
                    .kill_on_drop(true)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status(),
                action,
            )
            .await?;
        if status.success() {
            Ok(())
        } else {
            Err(BuildError::CommandFailed(action))
        }
    }

    async fn run_logged(
        &self,
        command: &mut Command,
        action: &'static str,
        logs: &DeploymentLogSink,
    ) -> Result<(), BuildError> {
        logs.system(format!("Starting {action}")).await?;
        let mut child = command
            .kill_on_drop(true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| command_io_error(action, error))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| BuildError::CommandFailed(action))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| BuildError::CommandFailed(action))?;
        let (_, _, status) = tokio::time::timeout(self.command_timeout, async {
            tokio::try_join!(
                stream_command_output(stdout, "stdout", logs),
                stream_command_output(stderr, "stderr", logs),
                async { child.wait().await.map_err(BuildError::Io) },
            )
        })
        .await
        .map_err(|_| BuildError::CommandTimedOut(action))??;
        if status.success() {
            logs.system(format!("Completed {action}")).await?;
            Ok(())
        } else {
            let code = status
                .code()
                .map_or_else(|| "signal".to_owned(), |code| format!("exit code {code}"));
            logs.system(format!("{action} failed ({code})")).await?;
            Err(BuildError::CommandFailed(action))
        }
    }

    async fn output(
        &self,
        command: &mut Command,
        action: &'static str,
    ) -> Result<String, BuildError> {
        let output = self
            .wait_for_command(
                command.kill_on_drop(true).stderr(Stdio::null()).output(),
                action,
            )
            .await?;
        if !output.status.success() {
            return Err(BuildError::CommandFailed(action));
        }
        String::from_utf8(output.stdout)
            .map(|value| value.trim().to_owned())
            .map_err(|_| BuildError::CommandFailed(action))
    }

    fn git_command(&self, credentials_path: &Path) -> Command {
        let mut command = Command::new(&self.git_bin);
        let hooks_path = if cfg!(windows) { "NUL" } else { "/dev/null" };
        command
            .env("GIT_CONFIG_GLOBAL", credentials_path)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .args(["-c", "protocol.file.allow=never"])
            .arg("-c")
            .arg(format!("core.hooksPath={hooks_path}"));
        command
    }

    async fn wait_for_command<T>(
        &self,
        future: impl std::future::Future<Output = std::io::Result<T>>,
        action: &'static str,
    ) -> Result<T, BuildError> {
        tokio::time::timeout(self.command_timeout, future)
            .await
            .map_err(|_| BuildError::CommandTimedOut(action))?
            .map_err(|error| command_io_error(action, error))
    }
}

async fn stream_command_output<R>(
    output: R,
    stream: &str,
    logs: &DeploymentLogSink,
) -> Result<(), BuildError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(output).lines();
    while let Some(line) = lines.next_line().await? {
        logs.append(stream, line).await?;
    }
    Ok(())
}

fn command_io_error(action: &'static str, error: std::io::Error) -> BuildError {
    if error.kind() == std::io::ErrorKind::NotFound {
        BuildError::CommandUnavailable(action)
    } else {
        BuildError::Io(error)
    }
}

impl SourceBuild for GitSourceBuild {
    async fn build(
        &self,
        deployment: &ignitify_db::DeploymentRecord,
        logs: &DeploymentLogSink,
    ) -> Result<Option<SourceBuildOutput>, ControlError> {
        let is_application_build = deployment
            .source_config
            .as_ref()
            .is_some_and(|source| source.source == "application");
        if is_application_build {
            logs.system("Waiting for source build capacity").await?;
            let _permit = self
                .build_limiter
                .acquire(&self.database)
                .await
                .map_err(|error| {
                    tracing::warn!(deployment_id = %deployment.id, error = %error, "could not acquire source build capacity");
                    ControlError::Policy("source build capacity is unavailable")
                })?;
            return self.build_inner(deployment, logs).await.map_err(|error| {
                tracing::warn!(deployment_id = %deployment.id, error = %error, "Git source build rejected");
                source_build_error(error)
            });
        }
        self.build_inner(deployment, logs).await.map_err(|error| {
            tracing::warn!(deployment_id = %deployment.id, error = %error, "Git source build rejected");
            source_build_error(error)
        })
    }
}

#[derive(Debug, Deserialize)]
struct StoredCredentials {
    token: Option<String>,
    private_key: Option<String>,
}

struct GitCredentials {
    username: String,
    token: String,
}

struct Checkout {
    path: PathBuf,
    revision: String,
}

struct RemoteBuildSession {
    name: String,
    certificate_dir: PathBuf,
}

struct RemoteBuildContext<'a> {
    session: &'a RemoteBuildSession,
    image: &'a str,
    metadata: &'a Path,
    checkout: &'a Path,
    logs: &'a DeploymentLogSink,
}

fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn command_timeout() -> std::time::Duration {
    let seconds = env_value("IGNITIFY_SOURCE_BUILD_TIMEOUT_SECONDS")
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| (60..=3_600).contains(seconds))
        .unwrap_or(DEFAULT_COMMAND_TIMEOUT_SECONDS);
    std::time::Duration::from_secs(seconds)
}

fn configured_digest_image(name: &'static str, value: &str) -> Result<String, BuildError> {
    if !is_digest_image_reference(value) {
        return Err(BuildError::InvalidImageSetting(name));
    }
    Ok(value.to_owned())
}

fn git_config(credentials: &GitCredentials) -> String {
    let authorization = STANDARD.encode(format!("{}:{}", credentials.username, credentials.token));
    format!(
        "[http]\n\textraHeader = Authorization: Basic {authorization}\n[credential]\n\thelper =\n"
    )
}

fn default_railpack_bin() -> String {
    let binary = if cfg!(windows) {
        "railpack.exe"
    } else {
        "railpack"
    };
    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join(binary)))
        .filter(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| binary.to_owned())
}

async fn remove_dir_if_exists(path: &Path) -> Result<(), std::io::Error> {
    match fs::remove_dir_all(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

async fn set_sensitive_directory_permissions(path: &Path) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).await?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

async fn remote_image_reference(image: &str, metadata: &Path) -> Result<String, BuildError> {
    let metadata = fs::read(metadata).await?;
    let metadata: serde_json::Value =
        serde_json::from_slice(&metadata).map_err(|_| BuildError::RemoteImageMetadata)?;
    let digest = metadata
        .get("containerimage.digest")
        .and_then(serde_json::Value::as_str)
        .filter(|digest| is_local_image_id(digest))
        .ok_or(BuildError::RemoteImageMetadata)?;
    Ok(format!("{image}@{digest}"))
}

async fn cleanup_checkout(checkout: &Checkout) {
    if let Err(error) = remove_dir_if_exists(&checkout.path).await {
        tracing::warn!(path = %checkout.path.display(), error = %error, "could not remove Git checkout");
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
