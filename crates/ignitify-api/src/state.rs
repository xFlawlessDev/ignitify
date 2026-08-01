use std::sync::Arc;

use ignitify_auth::AuthService;
use ignitify_control_plane::{ControlHandle, RuntimeHealth, ServiceControl};
use ignitify_db::Database;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) auth: Arc<AuthService>,
    pub(crate) database: Database,
    pub(crate) services: ServiceControl,
    pub(crate) control: ControlHandle,
    pub(crate) runtime_health: Arc<dyn RuntimeHealth>,
    pub(crate) worker_health: Arc<dyn RuntimeHealth>,
    pub(crate) secure_cookies: bool,
    pub(crate) trusted_origins: Arc<[String]>,
}
