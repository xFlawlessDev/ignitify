use std::{collections::HashMap, sync::Arc, time::Instant};

use ignitify_auth::AuthService;
use ignitify_control_plane::{
    AgeCipher, ControlHandle, RuntimeHealth, ServiceControl, SystemMetricsProvider,
};
use ignitify_db::Database;
use ignitify_runtime_docker::DockerRuntime;
use ignitify_terminal::TerminalService;
use tokio::sync::Mutex;

use crate::{DomainPolicy, error::ApiError};

pub(crate) const GITHUB_MANIFEST_STATE_TTL: std::time::Duration =
    std::time::Duration::from_secs(60 * 60);

#[derive(Debug)]
pub(crate) struct GithubManifestPending {
    pub(crate) user_id: String,
    pub(crate) name: String,
    pub(crate) base_url: String,
    pub(crate) frontend_origin: String,
    pub(crate) created_at: Instant,
}

pub(crate) type GithubManifestStates = Arc<Mutex<HashMap<String, GithubManifestPending>>>;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) auth: Arc<AuthService>,
    pub(crate) database: Database,
    pub(crate) services: Option<ServiceControl>,
    pub(crate) control: Option<ControlHandle>,
    pub(crate) runtime_health: Arc<dyn RuntimeHealth>,
    pub(crate) worker_health: Arc<dyn RuntimeHealth>,
    pub(crate) ingress_health: Arc<dyn RuntimeHealth>,
    pub(crate) system_metrics: Arc<dyn SystemMetricsProvider>,
    pub(crate) docker_runtime: Option<DockerRuntime>,
    pub(crate) terminal: TerminalService,
    pub(crate) secure_cookies: bool,
    pub(crate) trusted_origins: Arc<[String]>,
    pub(crate) provider_cipher: Option<Arc<AgeCipher>>,
    pub(crate) domain_policy: DomainPolicy,
    pub(crate) github_manifest_states: GithubManifestStates,
}

impl AppState {
    pub(crate) fn services(&self) -> Result<&ServiceControl, ApiError> {
        self.services
            .as_ref()
            .ok_or(ApiError::CapabilityUnavailable)
    }

    pub(crate) fn control(&self) -> Result<&ControlHandle, ApiError> {
        self.control.as_ref().ok_or(ApiError::CapabilityUnavailable)
    }

    pub(crate) fn docker_runtime(&self) -> Result<&DockerRuntime, ApiError> {
        self.docker_runtime
            .as_ref()
            .ok_or(ApiError::DockerCapabilityUnavailable)
    }
}
