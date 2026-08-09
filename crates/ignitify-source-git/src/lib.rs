//! Isolated Git checkout and image build adapter for application sources.

use std::{
    env,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ignitify_control_plane::{AgeCipher, Error as ControlError, SourceBuild, SourceBuildOutput};
use ignitify_db::{
    Database, ProviderAuthMode, ProviderKind, ProviderRecord, RemoteBuilderConnection,
};
use ignitify_domain::{
    ApplicationBuilder, ServiceSourceConfig, ServiceSpec, is_digest_image_reference,
};
use serde::Deserialize;
use thiserror::Error;
#[cfg(unix)]
use tokio::io::AsyncWriteExt;
use tokio::{fs, process::Command};
use url::Url;

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
    ) -> Result<Option<SourceBuildOutput>, BuildError> {
        let Some(source) = deployment.source_config.as_ref() else {
            return Ok(None);
        };
        match source.source.as_str() {
            "application" => self.build_application(deployment, source).await,
            "compose" if source.provider_id.is_some() => {
                self.build_compose(deployment, source).await
            }
            _ => Ok(None),
        }
    }

    async fn build_application(
        &self,
        deployment: &ignitify_db::DeploymentRecord,
        source: &ServiceSourceConfig,
    ) -> Result<Option<SourceBuildOutput>, BuildError> {
        let builder = source.builder.ok_or(BuildError::InvalidSource)?;
        if builder == ApplicationBuilder::Spa {
            return Err(BuildError::UnsupportedBuilder);
        }
        if builder == ApplicationBuilder::Static && deployment.spec.internal_port() != Some(80) {
            return Err(BuildError::StaticPort);
        }
        let checkout = self.checkout_source(deployment, source).await?;
        let result = self
            .build_image(deployment.id.as_str(), builder, source, &checkout.path)
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
    ) -> Result<Option<SourceBuildOutput>, BuildError> {
        let checkout = self.checkout_source(deployment, source).await?;
        let result = self.compose_spec(deployment, source, &checkout.path).await;
        cleanup_checkout(&checkout).await;
        Ok(Some(SourceBuildOutput {
            source_revision: checkout.revision,
            local_image_id: None,
            runtime_spec: Some(result?),
        }))
    }

    async fn checkout_source(
        &self,
        deployment: &ignitify_db::DeploymentRecord,
        source: &ServiceSourceConfig,
    ) -> Result<Checkout, BuildError> {
        let provider_id = source
            .provider_id
            .as_deref()
            .ok_or(BuildError::InvalidSource)?;
        let repository = source
            .repository
            .as_deref()
            .ok_or(BuildError::InvalidSource)?;
        let branch = source.branch.as_deref().ok_or(BuildError::InvalidSource)?;
        let provider = self
            .database
            .providers()
            .get(provider_id)
            .await?
            .ok_or(BuildError::ProviderMissing)?;
        let credentials = self.credentials(&provider)?;
        self.checkout(
            deployment.id.as_str(),
            &provider,
            &credentials,
            repository,
            branch,
            deployment.source_revision.as_deref(),
        )
        .await
    }

    async fn compose_spec(
        &self,
        deployment: &ignitify_db::DeploymentRecord,
        source: &ServiceSourceConfig,
        checkout: &Path,
    ) -> Result<ServiceSpec, BuildError> {
        let compose_path = relative_path(
            source
                .dockerfile_path
                .as_deref()
                .unwrap_or("docker-compose.yml"),
        )?;
        let yaml = fs::read_to_string(checkout.join(compose_path)).await?;
        compose_runtime_spec(&deployment.spec, yaml)
    }

    fn credentials(&self, provider: &ProviderRecord) -> Result<GitCredentials, BuildError> {
        if provider.auth_mode == ProviderAuthMode::GithubApp {
            return Err(BuildError::GithubAppUnsupported);
        }
        let plaintext = self
            .cipher
            .decrypt(&provider.credentials_ciphertext)
            .map_err(|_| BuildError::CredentialsUnavailable)?;
        let token = match serde_json::from_slice::<StoredCredentials>(plaintext.as_slice()) {
            Ok(credentials) => credentials.token,
            Err(_) => String::from_utf8(plaintext.to_vec()).ok(),
        }
        .filter(|token| !token.trim().is_empty())
        .ok_or(BuildError::CredentialsUnavailable)?;
        let username = provider
            .username
            .clone()
            .unwrap_or_else(|| match provider.kind {
                ProviderKind::Github => "x-access-token".to_owned(),
                ProviderKind::Gitlab => "oauth2".to_owned(),
                ProviderKind::Gitea | ProviderKind::Git => "git".to_owned(),
            });
        Ok(GitCredentials { username, token })
    }

    async fn checkout(
        &self,
        deployment_id: &str,
        provider: &ProviderRecord,
        credentials: &GitCredentials,
        repository: &str,
        branch: &str,
        source_revision: Option<&str>,
    ) -> Result<Checkout, BuildError> {
        fs::create_dir_all(&self.root).await?;
        let path = self.root.join(deployment_id);
        let credentials_path = self.root.join(format!("{deployment_id}.gitconfig"));
        remove_dir_if_exists(&path).await?;
        remove_file_if_exists(&credentials_path).await?;
        let remote = repository_url(provider, repository)?;
        write_credentials_config(&credentials_path, &git_config(credentials)).await?;
        let credentials_include = format!("include.path={}", credentials_path.display());
        let clone_result = self
            .run(
                self.git_command(&credentials_include)
                    .args([
                        "clone",
                        "--depth",
                        "1",
                        "--no-tags",
                        "--single-branch",
                        "--no-recurse-submodules",
                        "--branch",
                        branch,
                    ])
                    .arg(&remote)
                    .arg(&path),
                "git checkout",
            )
            .await;
        if let Err(error) = clone_result {
            let _ = fs::remove_file(&credentials_path).await;
            return Err(error);
        }
        if let Some(revision) = source_revision {
            if let Err(error) = self
                .run(
                    self.git_command(&credentials_include).args([
                        "-C",
                        path.to_string_lossy().as_ref(),
                        "fetch",
                        "--depth",
                        "1",
                        "origin",
                        revision,
                    ]),
                    "git revision fetch",
                )
                .await
            {
                let _ = fs::remove_file(&credentials_path).await;
                return Err(error);
            }
            if let Err(error) = self
                .run(
                    self.git_command(&credentials_include).args([
                        "-C",
                        path.to_string_lossy().as_ref(),
                        "checkout",
                        "--detach",
                        "FETCH_HEAD",
                    ]),
                    "git revision checkout",
                )
                .await
            {
                let _ = fs::remove_file(&credentials_path).await;
                return Err(error);
            }
        }
        let revision = self
            .output(
                self.git_command(&credentials_include).args([
                    "-C",
                    path.to_string_lossy().as_ref(),
                    "rev-parse",
                    "HEAD",
                ]),
                "git revision",
            )
            .await;
        let _ = fs::remove_file(&credentials_path).await;
        let revision = revision?;
        if !is_git_revision(&revision) {
            return Err(BuildError::InvalidRevision);
        }
        Ok(Checkout { path, revision })
    }

    async fn build_image(
        &self,
        deployment_id: &str,
        builder: ApplicationBuilder,
        source: &ServiceSourceConfig,
        checkout: &Path,
    ) -> Result<String, BuildError> {
        if let Some(remote) = self.database.remote_builders().active().await? {
            return self
                .build_remote_image(deployment_id, builder, source, checkout, &remote)
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
                self.docker_build(&tag, checkout, checkout.join(dockerfile))
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
                self.run(&mut prepare, "railpack prepare").await?;
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
                self.run(&mut build, "railpack image build").await?;
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
                self.docker_build(&tag, checkout, dockerfile).await?;
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
        self.run(&mut command, "docker image build").await
    }

    async fn build_remote_image(
        &self,
        deployment_id: &str,
        builder: ApplicationBuilder,
        source: &ServiceSourceConfig,
        checkout: &Path,
        remote: &RemoteBuilderConnection,
    ) -> Result<String, BuildError> {
        let session = self.open_remote_builder(deployment_id, remote).await?;
        let image = format!("{}:ignitify-{}", remote.registry_repository, deployment_id);
        let metadata = checkout.join(".ignitify-build-metadata.json");
        let result = self
            .build_remote_image_with_session(&session, &image, &metadata, builder, source, checkout)
            .await;
        self.close_remote_builder(session).await;
        result
    }

    async fn build_remote_image_with_session(
        &self,
        session: &RemoteBuildSession,
        image: &str,
        metadata: &Path,
        builder: ApplicationBuilder,
        source: &ServiceSourceConfig,
        checkout: &Path,
    ) -> Result<String, BuildError> {
        match builder {
            ApplicationBuilder::Dockerfile => {
                let dockerfile =
                    relative_path(source.dockerfile_path.as_deref().unwrap_or("Dockerfile"))?;
                self.remote_docker_build(
                    session,
                    image,
                    metadata,
                    checkout,
                    checkout.join(dockerfile),
                )
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
                self.run(&mut prepare, "railpack prepare").await?;
                let mut build = Command::new(&self.docker_bin);
                build
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
                    .arg("--build-arg")
                    .arg(format!("BUILDKIT_SYNTAX={frontend}"))
                    .arg("--file")
                    .arg(plan)
                    .arg(checkout);
                self.run(&mut build, "remote railpack image build").await?;
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
                self.remote_docker_build(session, image, metadata, checkout, dockerfile)
                    .await?;
            }
            ApplicationBuilder::Spa => return Err(BuildError::UnsupportedBuilder),
        }
        remote_image_reference(image, metadata).await
    }

    async fn remote_docker_build(
        &self,
        session: &RemoteBuildSession,
        image: &str,
        metadata: &Path,
        checkout: &Path,
        dockerfile: PathBuf,
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
        self.run(&mut command, "remote Docker image build").await
    }

    async fn open_remote_builder(
        &self,
        deployment_id: &str,
        remote: &RemoteBuilderConnection,
    ) -> Result<RemoteBuildSession, BuildError> {
        let certificate_dir = self.root.join(format!("{deployment_id}.remote-builder"));
        remove_dir_if_exists(&certificate_dir).await?;
        fs::create_dir_all(&certificate_dir).await?;
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
            self.run(&mut create, "remote builder connection").await?;
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

    fn git_command(&self, credentials_include: &str) -> Command {
        let mut command = Command::new(&self.git_bin);
        let hooks_path = if cfg!(windows) { "NUL" } else { "/dev/null" };
        command
            .args(["-c", "protocol.file.allow=never"])
            .arg("-c")
            .arg(format!("core.hooksPath={hooks_path}"))
            .arg("-c")
            .arg(credentials_include);
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
            .map_err(BuildError::Io)
    }
}

impl SourceBuild for GitSourceBuild {
    async fn build(
        &self,
        deployment: &ignitify_db::DeploymentRecord,
    ) -> Result<Option<SourceBuildOutput>, ControlError> {
        let is_application_build = deployment
            .source_config
            .as_ref()
            .is_some_and(|source| source.source == "application");
        if is_application_build {
            let _permit = self
                .build_limiter
                .acquire(&self.database)
                .await
                .map_err(|error| {
                    tracing::warn!(deployment_id = %deployment.id, error = %error, "could not acquire source build capacity");
                    ControlError::Policy("source build capacity is unavailable")
                })?;
            return self.build_inner(deployment).await.map_err(|error| {
                tracing::warn!(deployment_id = %deployment.id, error = %error, "Git source build rejected");
                ControlError::Policy("source build failed")
            });
        }
        self.build_inner(deployment).await.map_err(|error| {
            tracing::warn!(deployment_id = %deployment.id, error = %error, "Git source build rejected");
            ControlError::Policy("source build failed")
        })
    }
}

#[derive(Debug, Deserialize)]
struct StoredCredentials {
    token: Option<String>,
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

#[derive(Default)]
struct BuildLimiter {
    active: AtomicUsize,
    changed: tokio::sync::Notify,
}

impl BuildLimiter {
    async fn acquire(self: &Arc<Self>, database: &Database) -> Result<BuildPermit, BuildError> {
        loop {
            let limit = usize::try_from(database.server_settings().get().await?.concurrent_builds)
                .map_err(|_| BuildError::InvalidConcurrentBuildLimit)?;
            let active = self.active.load(Ordering::Acquire);
            if active < limit
                && self
                    .active
                    .compare_exchange(active, active + 1, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
            {
                return Ok(BuildPermit {
                    limiter: self.clone(),
                });
            }
            tokio::select! {
                _ = self.changed.notified() => {}
                _ = tokio::time::sleep(Duration::from_millis(250)) => {}
            }
        }
    }
}

struct BuildPermit {
    limiter: Arc<BuildLimiter>,
}

impl Drop for BuildPermit {
    fn drop(&mut self) {
        self.limiter.active.fetch_sub(1, Ordering::Release);
        self.limiter.changed.notify_waiters();
    }
}

#[derive(Debug, Error)]
enum BuildError {
    #[error("application source is incomplete")]
    InvalidSource,
    #[error("SPA source builds are not supported")]
    UnsupportedBuilder,
    #[error("a remote builder is required because local Docker builds are disabled")]
    LocalBuilderDisabled,
    #[error("static source builds require internal port 80")]
    StaticPort,
    #[error("Git Compose source requires a Compose service configuration")]
    InvalidComposeSource,
    #[error("source provider is missing")]
    ProviderMissing,
    #[error("provider credentials are unavailable")]
    CredentialsUnavailable,
    #[error("GitHub App credentials are not supported by the Git executor")]
    GithubAppUnsupported,
    #[error("source repository URL is invalid")]
    InvalidRepositoryUrl,
    #[error("source path must stay inside the repository")]
    UnsafePath,
    #[error("source revision is invalid")]
    InvalidRevision,
    #[error("stored concurrent build limit is invalid")]
    InvalidConcurrentBuildLimit,
    #[error("built image ID is invalid")]
    InvalidImageId,
    #[error("remote builder did not return an image digest")]
    RemoteImageMetadata,
    #[error("IGNITIFY_STATIC_RUNTIME_IMAGE must be a digest-pinned image")]
    InvalidStaticRuntimeImage,
    #[error("{0} must be a digest-pinned image")]
    InvalidImageSetting(&'static str),
    #[error("{0} failed")]
    CommandFailed(&'static str),
    #[error("{0} exceeded the configured build timeout")]
    CommandTimedOut(&'static str),
    #[error(transparent)]
    Database(#[from] ignitify_db::DatabaseError),
    #[error(transparent)]
    Control(#[from] ControlError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
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

fn repository_url(provider: &ProviderRecord, repository: &str) -> Result<String, BuildError> {
    let base = provider
        .internal_url
        .as_deref()
        .unwrap_or(&provider.base_url)
        .trim_end_matches('/');
    let url = Url::parse(base).map_err(|_| BuildError::InvalidRepositoryUrl)?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(BuildError::InvalidRepositoryUrl);
    }
    if provider.kind == ProviderKind::Git && base.ends_with(".git") {
        return Ok(base.to_owned());
    }
    let repository = repository.trim_matches('/');
    if repository.is_empty()
        || repository
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(BuildError::InvalidRepositoryUrl);
    }
    Ok(format!("{base}/{repository}.git"))
}

fn git_config(credentials: &GitCredentials) -> String {
    let authorization = STANDARD.encode(format!("{}:{}", credentials.username, credentials.token));
    format!(
        "[http]\n\textraHeader = Authorization: Basic {authorization}\n[credential]\n\thelper =\n"
    )
}

fn relative_path(value: &str) -> Result<PathBuf, BuildError> {
    let path = Path::new(value);
    if path.is_absolute()
        || value.is_empty()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(BuildError::UnsafePath);
    }
    Ok(path.to_path_buf())
}

fn compose_runtime_spec(
    deployment_spec: &ServiceSpec,
    yaml: String,
) -> Result<ServiceSpec, BuildError> {
    let ServiceSpec::Compose {
        exposed_service,
        internal_port,
        ..
    } = deployment_spec
    else {
        return Err(BuildError::InvalidComposeSource);
    };
    ServiceSpec::compose(yaml, exposed_service, *internal_port)
        .map_err(|_| BuildError::InvalidComposeSource)
}

fn static_dockerfile(
    build_image: &str,
    runtime_image: &str,
    command: &str,
    output: &Path,
) -> String {
    format!(
        "FROM {build_image} AS build\nWORKDIR /app\nCOPY . .\nRUN /bin/sh -ec {}\nFROM {runtime_image}\nCOPY --from=build /app/{} /usr/share/caddy\n",
        shell_quote(command),
        output.display(),
    )
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn is_git_revision(value: &str) -> bool {
    (40..=128).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_local_image_id(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

async fn remove_dir_if_exists(path: &Path) -> Result<(), std::io::Error> {
    match fs::remove_dir_all(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

async fn remove_file_if_exists(path: &Path) -> Result<(), std::io::Error> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
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

async fn write_credentials_config(path: &Path, contents: &str) -> Result<(), std::io::Error> {
    write_sensitive_file(path, contents.as_bytes()).await
}

async fn write_sensitive_file(path: &Path, contents: &[u8]) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .await?;
        file.write_all(contents).await?;
        file.flush().await?;
    }
    #[cfg(not(unix))]
    {
        fs::write(path, contents).await?;
    }
    Ok(())
}

async fn cleanup_checkout(checkout: &Checkout) {
    if let Err(error) = remove_dir_if_exists(&checkout.path).await {
        tracing::warn!(path = %checkout.path.display(), error = %error, "could not remove Git checkout");
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BuildLimiter, compose_runtime_spec, is_git_revision, is_local_image_id, relative_path,
        shell_quote, static_dockerfile,
    };
    use ignitify_db::{Database, DatabaseConfig};
    use ignitify_domain::ServiceSpec;
    use std::sync::Arc;

    #[test]
    fn static_build_uses_the_generated_dockerfile_not_the_host_shell() {
        let dockerfile = static_dockerfile(
            "node@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "caddy@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "npm ci && npm run build",
            std::path::Path::new("dist"),
        );
        assert!(dockerfile.contains("RUN /bin/sh -ec 'npm ci && npm run build'"));
        assert!(dockerfile.contains("COPY --from=build /app/dist /usr/share/caddy"));
    }

    #[tokio::test]
    async fn build_limiter_holds_the_configured_number_of_slots() {
        let database = Database::connect(&DatabaseConfig {
            url: "sqlite::memory:".to_owned(),
        })
        .await
        .unwrap();
        let limiter = Arc::new(BuildLimiter::default());
        let first = limiter.acquire(&database).await.unwrap();
        let second = limiter.acquire(&database).await.unwrap();
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(20),
                limiter.acquire(&database),
            )
            .await
            .is_err()
        );
        drop(first);
        let third = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            limiter.acquire(&database),
        )
        .await
        .unwrap()
        .unwrap();
        drop((second, third));
    }

    #[test]
    fn source_paths_cannot_escape_the_checkout() {
        assert!(relative_path("Dockerfile").is_ok());
        assert!(relative_path("apps/web/dist").is_ok());
        assert!(relative_path("../Dockerfile").is_err());
        assert!(relative_path("/etc/passwd").is_err());
    }

    #[test]
    fn git_compose_uses_checked_out_yaml_with_configured_routing() {
        let configured = ServiceSpec::compose(
            "services:\n  web:\n    image: nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
            "web",
            Some(8080),
        )
        .unwrap();
        let runtime = compose_runtime_spec(
            &configured,
            "services:\n  web:\n    image: caddy@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n"
                .to_owned(),
        )
        .unwrap();
        let ServiceSpec::Compose {
            yaml,
            exposed_service,
            internal_port,
        } = runtime
        else {
            panic!("expected Compose runtime specification");
        };
        assert!(yaml.contains("caddy@sha256:"));
        assert_eq!(exposed_service, "web");
        assert_eq!(internal_port, Some(8080));
    }

    #[test]
    fn revision_and_local_image_ids_have_strict_grammars() {
        assert!(is_git_revision(&"a".repeat(40)));
        assert!(!is_git_revision("main"));
        assert!(is_local_image_id(&format!("sha256:{}", "b".repeat(64))));
        assert!(!is_local_image_id("sha256:short"));
    }

    #[test]
    fn shell_quoting_keeps_user_command_inside_one_argument() {
        assert_eq!(shell_quote("echo 'ok'"), "'echo '\"'\"'ok'\"'\"''");
    }
}
