use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use ignitify_control_plane::Error as ControlError;
use ignitify_db::Database;
use thiserror::Error;

use crate::{command_failure, github_app};

#[derive(Default)]
pub(crate) struct BuildLimiter {
    active: AtomicUsize,
    changed: tokio::sync::Notify,
}

impl BuildLimiter {
    pub(crate) async fn acquire(
        self: &Arc<Self>,
        database: &Database,
    ) -> Result<BuildPermit, BuildError> {
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

pub(crate) struct BuildPermit {
    limiter: Arc<BuildLimiter>,
}

impl Drop for BuildPermit {
    fn drop(&mut self) {
        self.limiter.active.fetch_sub(1, Ordering::Release);
        self.limiter.changed.notify_waiters();
    }
}

#[derive(Debug, Error)]
pub(crate) enum BuildError {
    #[error("application source is incomplete")]
    InvalidSource,
    #[error("SPA source builds are not supported")]
    UnsupportedBuilder,
    #[error("a remote builder is required because local Docker builds are disabled")]
    LocalBuilderDisabled,
    #[error("remote source deployments require a configured remote builder")]
    RemoteBuilderRequired,
    #[error("static source builds require internal port 80")]
    StaticPort,
    #[error("Git Compose source must define at least one valid Compose service")]
    InvalidComposeSource,
    #[error("source provider is missing")]
    ProviderMissing,
    #[error("provider credentials are unavailable")]
    CredentialsUnavailable,
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
    #[error("Git source checkout failed: {0}")]
    GitCheckout(command_failure::GitCheckoutFailure),
    #[error("{0} executable is unavailable")]
    CommandUnavailable(&'static str),
    #[error("{0} exceeded the configured build timeout")]
    CommandTimedOut(&'static str),
    #[error(transparent)]
    GithubApp(#[from] github_app::Error),
    #[error(transparent)]
    Database(#[from] ignitify_db::DatabaseError),
    #[error(transparent)]
    Control(#[from] ControlError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub(crate) fn source_build_error(error: BuildError) -> ControlError {
    match error {
        BuildError::RemoteBuilderRequired => ControlError::Policy(
            "remote source deployments require a configured remote builder and registry",
        ),
        error => ControlError::SourceBuild(source_build_reason(&error)),
    }
}

fn source_build_reason(error: &BuildError) -> String {
    match error {
        BuildError::CommandUnavailable(action) if action.contains("railpack") => {
            "Railpack CLI is not installed on the control-plane host. Install it or set IGNITIFY_RAILPACK_BIN to its absolute path.".to_owned()
        }
        BuildError::CommandUnavailable(action) if action.contains("docker") => {
            "Docker CLI is not installed on the control-plane host or is not available in PATH."
                .to_owned()
        }
        BuildError::CommandUnavailable(action) if action.contains("git") => {
            "Git CLI is not installed on the control-plane host or is not available in PATH."
                .to_owned()
        }
        BuildError::CommandUnavailable(action) => {
            format!("{action} could not start because its executable is unavailable.")
        }
        BuildError::CommandFailed(action) => {
            format!("{action} failed. Check the source configuration and build tool prerequisites.")
        }
        BuildError::GitCheckout(error) => format!("Git source checkout failed: {error}"),
        BuildError::CommandTimedOut(action) => format!("{action} exceeded the configured build timeout."),
        BuildError::GithubApp(error) => format!("GitHub App authentication failed: {error}"),
        error => error.to_string(),
    }
}
