use std::sync::Arc;

use ignitify_auth::AuthService;
use ignitify_db::Database;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) auth: Arc<AuthService>,
    pub(crate) database: Database,
    pub(crate) secure_cookies: bool,
    pub(crate) trusted_origins: Arc<[String]>,
}
