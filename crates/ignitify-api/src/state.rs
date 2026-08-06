use std::sync::Arc;

use ignitify_auth::AuthService;
use ignitify_control_plane::{ControlHandle, RuntimeHealth, ServiceControl, SystemMetricsProvider};
use ignitify_db::Database;
use ignitify_runtime_docker::DockerRuntime;
use ignitify_terminal::TerminalService;

use crate::error::ApiError;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) auth: Arc<AuthService>,
    pub(crate) database: Database,
    pub(crate) services: Option<ServiceControl>,
    pub(crate) control: Option<ControlHandle>,
    pub(crate) runtime_health: Arc<dyn RuntimeHealth>,
    pub(crate) worker_health: Arc<dyn RuntimeHealth>,
    pub(crate) system_metrics: Arc<dyn SystemMetricsProvider>,
    pub(crate) docker_runtime: Option<DockerRuntime>,
    pub(crate) terminal: TerminalService,
    pub(crate) secure_cookies: bool,
    pub(crate) trusted_origins: Arc<[String]>,
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
