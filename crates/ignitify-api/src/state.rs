use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use ignitify_auth::AuthService;
use ignitify_control_plane::{
    AgeCipher, ControlHandle, RuntimeHealth, ServiceControl, SystemMetricsProvider,
};
use ignitify_db::Database;
use ignitify_runtime_docker::DockerRuntime;
use ignitify_terminal::TerminalService;
use tokio::sync::{Mutex, Semaphore};

use crate::{DomainPolicy, error::ApiError};

pub(crate) const GITHUB_MANIFEST_STATE_TTL: std::time::Duration =
    std::time::Duration::from_secs(60 * 60);
const LOGIN_ATTEMPT_WINDOW: Duration = Duration::from_secs(15 * 60);
const MAX_LOGIN_ATTEMPTS: usize = 5;
const MAX_LOGIN_RATE_LIMIT_KEYS: usize = 4_096;
const AI_CHAT_WINDOW: Duration = Duration::from_secs(60);
const MAX_AI_CHAT_REQUESTS: usize = 20;
const MAX_AI_CHAT_RATE_LIMIT_KEYS: usize = 4_096;

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
pub(crate) struct OriginPolicy {
    inner: Arc<RwLock<OriginPolicyState>>,
}

struct OriginPolicyState {
    base_origins: Arc<[String]>,
    base_require_explicit_origin: bool,
    base_trust_proxy_headers: bool,
    control_plane_origin: Option<String>,
}

impl OriginPolicy {
    pub(crate) fn new(
        require_explicit_origin: bool,
        trust_proxy_headers: bool,
        base_origins: Arc<[String]>,
        control_plane_domain: Option<String>,
    ) -> Self {
        Self {
            inner: Arc::new(RwLock::new(OriginPolicyState {
                base_origins,
                base_require_explicit_origin: require_explicit_origin,
                base_trust_proxy_headers: trust_proxy_headers,
                control_plane_origin: control_plane_domain
                    .filter(|domain| !domain.is_empty())
                    .map(|domain| format!("https://{domain}")),
            })),
        }
    }

    pub(crate) fn is_trusted(&self, origin: &str) -> bool {
        self.inner.read().is_ok_and(|state| {
            state.base_origins.iter().any(|trusted| trusted == origin)
                || state.control_plane_origin.as_deref() == Some(origin)
        })
    }

    pub(crate) fn requires_explicit_origin(&self) -> bool {
        self.inner.read().map_or(true, |state| {
            state.base_require_explicit_origin || state.control_plane_origin.is_some()
        })
    }

    pub(crate) fn trusts_proxy_headers(&self) -> bool {
        self.inner.read().is_ok_and(|state| {
            state.base_trust_proxy_headers || state.control_plane_origin.is_some()
        })
    }

    pub(crate) fn public_origin(&self) -> Option<String> {
        self.inner.read().ok().and_then(|state| {
            state
                .control_plane_origin
                .clone()
                .or_else(|| state.base_origins.first().cloned())
        })
    }

    pub(crate) fn set_control_plane_domain(&self, domain: Option<String>) -> bool {
        self.inner.write().is_ok_and(|mut state| {
            state.control_plane_origin = domain
                .filter(|value| !value.is_empty())
                .map(|value| format!("https://{value}"));
            true
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct LoginRateLimiter {
    attempts: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
}

#[derive(Clone, Default)]
pub(crate) struct AiChatRateLimiter {
    attempts: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
}

impl AiChatRateLimiter {
    pub(crate) async fn allows(&self, user_id: &str) -> bool {
        let mut attempts = self.attempts.lock().await;
        let cutoff = Instant::now() - AI_CHAT_WINDOW;
        attempts.retain(|_, entries| {
            while entries.front().is_some_and(|attempt| *attempt <= cutoff) {
                entries.pop_front();
            }
            !entries.is_empty()
        });
        if !attempts.contains_key(user_id) && attempts.len() >= MAX_AI_CHAT_RATE_LIMIT_KEYS {
            return false;
        }
        let entries = attempts.entry(user_id.to_owned()).or_default();
        if entries.len() >= MAX_AI_CHAT_REQUESTS {
            return false;
        }
        entries.push_back(Instant::now());
        true
    }
}

impl LoginRateLimiter {
    pub(crate) async fn allows(&self, source: &str, username: &str) -> bool {
        let mut attempts = self.attempts.lock().await;
        prune_expired(&mut attempts);
        attempts
            .get(&rate_limit_key("source", source))
            .is_none_or(|entries| entries.len() < MAX_LOGIN_ATTEMPTS)
            && attempts
                .get(&rate_limit_key("username", username))
                .is_none_or(|entries| entries.len() < MAX_LOGIN_ATTEMPTS)
    }

    pub(crate) async fn record_failure(&self, source: &str, username: &str) {
        let mut attempts = self.attempts.lock().await;
        prune_expired(&mut attempts);
        record_attempt(&mut attempts, rate_limit_key("source", source));
        record_attempt(&mut attempts, rate_limit_key("username", username));
    }
}

fn prune_expired(attempts: &mut HashMap<String, VecDeque<Instant>>) {
    let cutoff = Instant::now() - LOGIN_ATTEMPT_WINDOW;
    attempts.retain(|_, entries| {
        while entries.front().is_some_and(|attempt| *attempt <= cutoff) {
            entries.pop_front();
        }
        !entries.is_empty()
    });
}

fn record_attempt(attempts: &mut HashMap<String, VecDeque<Instant>>, key: String) {
    if !attempts.contains_key(&key) && attempts.len() >= MAX_LOGIN_RATE_LIMIT_KEYS {
        return;
    }
    attempts.entry(key).or_default().push_back(Instant::now());
}

fn rate_limit_key(kind: &str, value: &str) -> String {
    format!("{kind}:{value}")
}

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
    pub(crate) host_terminal_enabled: bool,
    pub(crate) terminal_sessions: Arc<Semaphore>,
    pub(crate) login_rate_limiter: LoginRateLimiter,
    pub(crate) ai_chat_rate_limiter: AiChatRateLimiter,
    pub(crate) secure_cookies: bool,
    pub(crate) origin_policy: OriginPolicy,
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
