use std::{
    collections::BTreeMap,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, RwLock},
};

use ignitify_control_plane::{
    AgeCipher, Error as ControlError, Ingress, IngressRoute, Result as ControlResult, RuntimeHealth,
};
use ignitify_db::{Database, ServerSettingsRecord};
use ignitify_domain::{DomainId, DomainName, ServiceId};
use ignitify_runtime_docker::DockerRuntime;
use thiserror::Error;
use tokio::process::Command;
use uuid::Uuid;

pub const PROXY_NETWORK: &str = "ignitify-proxy";
pub const ENTRYPOINT: &str = "websecure";
pub const CERT_RESOLVER: &str = "le";
pub const INGRESS_LABEL: &str = "com.ignitify.ingress=traefik";

const FALLBACK_LABEL: &str = "com.ignitify.ingress=fallback";
const HTTP_ENTRYPOINT: &str = "web";
const TLS_REDIRECT_MIDDLEWARE: &str = "redirect-to-https@file";
const DYNAMIC_CERTIFICATES_FILE: &str = "certificates.yml";
const DYNAMIC_CERTIFICATES_DIR: &str = "certs";
const CONTROL_PLANE_ROUTE_FILE: &str = "control-plane.yml";
const TRAEFIK_DYNAMIC_DIRECTORY: &str = "/etc/traefik/dynamic";
const CONTROL_PLANE_UPSTREAM: &str = "http://host.docker.internal:5656";

#[derive(Clone)]
pub struct TraefikIngress {
    runtime: DockerRuntime,
    operator: OperatorConfig,
    routing_policy: Arc<RwLock<RoutingPolicy>>,
    operator_email: Arc<RwLock<Option<String>>>,
    server_settings: Option<ServerSettingsSource>,
}

#[derive(Clone)]
struct ServerSettingsSource {
    database: Database,
    cipher: Arc<AgeCipher>,
    dynamic_dir: PathBuf,
    fallback_page_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoutingPolicy {
    Http,
    Tls {
        certificate_resolver: Option<&'static str>,
    },
}

impl RoutingPolicy {
    fn from_settings(settings: &ServerSettingsRecord) -> Self {
        if !settings.https_enabled {
            return Self::Http;
        }
        let certificate_resolver = (settings.certificate_provider == "lets-encrypt"
            && settings.automatically_provision_ssl)
            .then_some(CERT_RESOLVER);
        Self::Tls {
            certificate_resolver,
        }
    }
}

impl Default for RoutingPolicy {
    fn default() -> Self {
        Self::Tls {
            certificate_resolver: Some(CERT_RESOLVER),
        }
    }
}

impl TraefikIngress {
    pub fn new(runtime: DockerRuntime) -> Self {
        Self {
            runtime,
            operator: OperatorConfig::from_environment(),
            routing_policy: Arc::new(RwLock::new(RoutingPolicy::default())),
            operator_email: Arc::new(RwLock::new(None)),
            server_settings: None,
        }
    }

    pub fn with_server_settings(
        runtime: DockerRuntime,
        database: Database,
        cipher: Arc<AgeCipher>,
    ) -> Self {
        Self {
            runtime,
            operator: OperatorConfig::from_environment(),
            routing_policy: Arc::new(RwLock::new(RoutingPolicy::default())),
            operator_email: Arc::new(RwLock::new(None)),
            server_settings: Some(ServerSettingsSource {
                database,
                cipher,
                dynamic_dir: dynamic_dir_from_environment(),
                fallback_page_path: fallback_page_path_from_environment(),
            }),
        }
    }

    pub async fn ready(&self) -> bool {
        let network = self.runtime.network_exists(PROXY_NETWORK).await;
        let ingress = self
            .runtime
            .has_running_container_with_label(INGRESS_LABEL)
            .await;
        let fallback = self
            .runtime
            .has_running_container_with_label(FALLBACK_LABEL)
            .await;
        matches!(network, Ok(true)) && matches!(ingress, Ok(true)) && matches!(fallback, Ok(true))
    }

    pub async fn ensure_started(&self) -> bool {
        let desired_email = self.desired_acme_email().await;
        if self.ready().await && self.operator_email_matches(&desired_email) {
            return true;
        }
        if !self.operator.auto_start {
            return self.ready().await;
        }
        if let Err(error) = self.operator.start(desired_email.as_deref()).await {
            tracing::warn!(error = %error, "could not start the Traefik operator stack");
            return false;
        }
        self.set_operator_email(desired_email);
        self.ready().await
    }

    async fn sync_server_settings(&self, source: &ServerSettingsSource) -> ControlResult<()> {
        let settings = source.database.server_settings().get().await?;
        sync_dynamic_certificates(source, &settings).await?;
        sync_control_plane_route(&source.dynamic_dir, &settings)
            .map_err(|_| ControlError::Runtime)?;
        write_fallback_page(&source.fallback_page_path, &settings)
            .map_err(|_| ControlError::Runtime)?;
        self.reconcile_operator_email(&settings.acme_email).await?;
        let policy = RoutingPolicy::from_settings(&settings);
        let mut current = self
            .routing_policy
            .write()
            .map_err(|_| ControlError::Runtime)?;
        *current = policy;
        Ok(())
    }

    async fn desired_acme_email(&self) -> Option<String> {
        let Some(source) = &self.server_settings else {
            return None;
        };
        match source.database.server_settings().get().await {
            Ok(settings) => normalized_email(&settings.acme_email),
            Err(error) => {
                tracing::warn!(error = %error, "could not load the ACME contact email");
                None
            }
        }
    }

    fn operator_email_matches(&self, desired: &Option<String>) -> bool {
        self.operator_email
            .read()
            .map(|current| current.as_ref() == desired.as_ref())
            .unwrap_or(false)
    }

    fn set_operator_email(&self, email: Option<String>) {
        if let Ok(mut current) = self.operator_email.write() {
            *current = email;
        }
    }

    async fn reconcile_operator_email(&self, email: &str) -> ControlResult<()> {
        let desired = normalized_email(email);
        if self.operator_email_matches(&desired) {
            return Ok(());
        }
        if self.operator.auto_start {
            self.operator
                .start(desired.as_deref())
                .await
                .map_err(|_| ControlError::Runtime)?;
        }
        self.set_operator_email(desired);
        Ok(())
    }
}

fn normalized_email(email: &str) -> Option<String> {
    let email = email.trim();
    (!email.is_empty()).then(|| email.to_owned())
}

#[derive(Clone)]
struct OperatorConfig {
    auto_start: bool,
    docker_bin: String,
    compose_file: PathBuf,
}

impl OperatorConfig {
    fn from_environment() -> Self {
        Self {
            auto_start: env::var("IGNITIFY_AUTO_START_INGRESS")
                .map(|value| value.trim() != "false")
                .unwrap_or(true),
            docker_bin: env::var("IGNITIFY_DOCKER_BIN")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "docker".to_owned()),
            compose_file: env::var("IGNITIFY_TRAEFIK_COMPOSE_FILE")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("infra/traefik/compose.yaml")),
        }
    }

    async fn start(&self, acme_email: Option<&str>) -> std::result::Result<(), OperatorError> {
        let compose_file = self
            .compose_file
            .file_name()
            .ok_or(OperatorError::InvalidComposePath)?;
        let working_dir = self
            .compose_file
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        if !self.compose_file.is_file() {
            return Err(OperatorError::ComposeFileMissing(self.compose_file.clone()));
        }
        let mut command = Command::new(&self.docker_bin);
        command
            .args(["compose", "-f"])
            .arg(compose_file)
            .args(["up", "-d"])
            .current_dir(working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(acme_email) = acme_email {
            command.env("IGNITIFY_ACME_EMAIL", acme_email);
        }
        let status = command.status().await.map_err(OperatorError::Command)?;
        if status.success() {
            Ok(())
        } else {
            Err(OperatorError::CommandFailed)
        }
    }
}

#[derive(Debug, Error)]
enum OperatorError {
    #[error("Traefik compose path is invalid")]
    InvalidComposePath,
    #[error("Traefik compose file is missing: {0}")]
    ComposeFileMissing(PathBuf),
    #[error("could not execute Docker Compose")]
    Command(#[source] std::io::Error),
    #[error("Docker Compose returned a failure status")]
    CommandFailed,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("internal route port must be between 1 and 65535")]
    InvalidPort,
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn render_route(
    domain_id: &DomainId,
    hostname: &DomainName,
    port: u32,
) -> Result<IngressRoute> {
    render_route_with_policy(domain_id, hostname, port, RoutingPolicy::default())
}

fn render_route_with_policy(
    domain_id: &DomainId,
    hostname: &DomainName,
    port: u32,
    policy: RoutingPolicy,
) -> Result<IngressRoute> {
    if !(1..=65_535).contains(&port) {
        return Err(Error::InvalidPort);
    }
    let name = format!("ignitify-{domain_id}");
    let router = format!("traefik.http.routers.{name}");
    let service = format!("traefik.http.services.{name}");
    let mut labels = BTreeMap::from([
        ("traefik.enable".to_owned(), "true".to_owned()),
        (format!("{router}.rule"), format!("Host(`{hostname}`)")),
        (format!("{router}.service"), name.clone()),
        (
            format!("{service}.loadbalancer.server.port"),
            port.to_string(),
        ),
    ]);
    match policy {
        RoutingPolicy::Http => {
            labels.insert(format!("{router}.entrypoints"), HTTP_ENTRYPOINT.to_owned());
        }
        RoutingPolicy::Tls {
            certificate_resolver,
        } => {
            labels.insert(format!("{router}.entrypoints"), ENTRYPOINT.to_owned());
            labels.insert(format!("{router}.tls"), "true".to_owned());
            if let Some(certificate_resolver) = certificate_resolver {
                labels.insert(
                    format!("{router}.tls.certresolver"),
                    certificate_resolver.to_owned(),
                );
            }
            let http_router = format!("traefik.http.routers.{name}-http");
            labels.insert(format!("{http_router}.rule"), format!("Host(`{hostname}`)"));
            labels.insert(
                format!("{http_router}.entrypoints"),
                HTTP_ENTRYPOINT.to_owned(),
            );
            labels.insert(
                format!("{http_router}.middlewares"),
                TLS_REDIRECT_MIDDLEWARE.to_owned(),
            );
            labels.insert(format!("{http_router}.service"), name);
        }
    }
    Ok(IngressRoute {
        labels,
        network: PROXY_NETWORK.to_owned(),
    })
}

fn dynamic_dir_from_environment() -> PathBuf {
    env::var("IGNITIFY_TRAEFIK_DYNAMIC_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("infra/traefik/dynamic"))
}

fn fallback_page_path_from_environment() -> PathBuf {
    if let Some(path) = env::var("IGNITIFY_TRAEFIK_FALLBACK_PAGE_FILE")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return PathBuf::from(path);
    }
    fallback_page_path_from_dynamic_dir(&dynamic_dir_from_environment())
}

fn fallback_page_path_from_dynamic_dir(dynamic_dir: &Path) -> PathBuf {
    dynamic_dir
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join("fallback")
        .join("404.html")
}

async fn sync_dynamic_certificates(
    source: &ServerSettingsSource,
    settings: &ServerSettingsRecord,
) -> ControlResult<()> {
    let selected_certificate = (settings.https_enabled
        && settings.certificate_provider == "custom")
        .then_some(settings.custom_certificate_id.as_deref())
        .flatten();
    let Some(certificate_id) = selected_certificate else {
        clear_dynamic_certificates(&source.dynamic_dir).map_err(|_| ControlError::Runtime)?;
        return Ok(());
    };
    if Uuid::parse_str(certificate_id).is_err() {
        return Err(ControlError::Runtime);
    }
    let certificate = source
        .database
        .server_settings()
        .certificate(certificate_id)
        .await?
        .ok_or(ControlError::Runtime)?;
    let certificate_contents = source.cipher.decrypt(&certificate.certificate_ciphertext)?;
    let private_key_contents = source.cipher.decrypt(&certificate.private_key_ciphertext)?;
    write_dynamic_certificates(
        &source.dynamic_dir,
        certificate_id,
        certificate_contents.as_slice(),
        private_key_contents.as_slice(),
    )
    .map_err(|_| ControlError::Runtime)
}

fn write_dynamic_certificates(
    dynamic_dir: &Path,
    certificate_id: &str,
    certificate: &[u8],
    private_key: &[u8],
) -> std::io::Result<()> {
    let certificates_dir = dynamic_dir.join(DYNAMIC_CERTIFICATES_DIR);
    clear_managed_certificate_files(&certificates_dir)?;
    let certificate_path = certificates_dir.join(format!("{certificate_id}.crt"));
    let private_key_path = certificates_dir.join(format!("{certificate_id}.key"));
    write_restricted(&certificate_path, certificate)?;
    write_restricted(&private_key_path, private_key)?;
    let content = format!(
        "tls:\n  certificates:\n    - certFile: {TRAEFIK_DYNAMIC_DIRECTORY}/{DYNAMIC_CERTIFICATES_DIR}/{certificate_id}.crt\n      keyFile: {TRAEFIK_DYNAMIC_DIRECTORY}/{DYNAMIC_CERTIFICATES_DIR}/{certificate_id}.key\n"
    );
    write_restricted(
        &dynamic_dir.join(DYNAMIC_CERTIFICATES_FILE),
        content.as_bytes(),
    )
}

fn clear_dynamic_certificates(dynamic_dir: &Path) -> std::io::Result<()> {
    remove_file_if_exists(&dynamic_dir.join(DYNAMIC_CERTIFICATES_FILE))?;
    clear_managed_certificate_files(&dynamic_dir.join(DYNAMIC_CERTIFICATES_DIR))
}

fn sync_control_plane_route(
    dynamic_dir: &Path,
    settings: &ServerSettingsRecord,
) -> std::io::Result<()> {
    let path = dynamic_dir.join(CONTROL_PLANE_ROUTE_FILE);
    let domain = settings.control_plane_domain.trim();
    if domain.is_empty() {
        return remove_file_if_exists(&path);
    }
    let hostname = DomainName::new(domain).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "control plane domain is invalid",
        )
    })?;
    let policy = RoutingPolicy::from_settings(settings);
    let RoutingPolicy::Tls {
        certificate_resolver,
    } = policy
    else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "control plane route requires TLS",
        ));
    };
    write_restricted(
        &path,
        render_control_plane_route(&hostname, certificate_resolver).as_bytes(),
    )
}

fn render_control_plane_route(hostname: &DomainName, certificate_resolver: Option<&str>) -> String {
    let certificate_resolver = certificate_resolver
        .map(|resolver| format!("\n        certResolver: {resolver}"))
        .unwrap_or_else(|| " {}".to_owned());
    format!(
        r#"http:
  routers:
    ignitify-control-plane:
      entryPoints:
        - websecure
      priority: 1000
      rule: "Host(`{hostname}`)"
      service: ignitify-control-plane
      tls:{}
    ignitify-control-plane-http:
      entryPoints:
        - web
      priority: 1000
      rule: "Host(`{hostname}`)"
      middlewares:
        - redirect-to-https@file
      service: ignitify-control-plane
  services:
    ignitify-control-plane:
      loadBalancer:
        servers:
          - url: "{CONTROL_PLANE_UPSTREAM}"
"#,
        certificate_resolver
    )
}

fn clear_managed_certificate_files(directory: &Path) -> std::io::Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if file_type.is_file()
            && (name.ends_with(".crt") || name.ends_with(".key") || name.ends_with(".tmp"))
        {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn write_restricted(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    write_with_permissions(path, contents, 0o700, 0o600)
}

fn write_public(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    write_with_permissions(path, contents, 0o755, 0o644)
}

fn write_with_permissions(
    path: &Path,
    contents: &[u8],
    directory_mode: u32,
    file_mode: u32,
) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "file path has no parent")
    })?;
    fs::create_dir_all(parent)?;
    set_permissions(parent, directory_mode)?;
    let temporary = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("new")
    ));
    {
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(file_mode);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(contents)?;
        file.sync_all()?;
    }
    set_permissions(&temporary, file_mode)?;
    #[cfg(windows)]
    remove_file_if_exists(path)?;
    fs::rename(temporary, path)?;
    set_permissions(path, file_mode)
}

fn write_fallback_page(path: &Path, settings: &ServerSettingsRecord) -> std::io::Result<()> {
    write_public(
        path,
        render_fallback_page(
            settings.fallback_page_heading.as_str(),
            settings.fallback_page_message.as_str(),
        )
        .as_bytes(),
    )
}

fn render_fallback_page(heading: &str, message: &str) -> String {
    let heading = escape_html(heading);
    let message = escape_html(message).replace('\n', "<br>");
    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="robots" content="noindex">
    <title>{heading} | Ignitify</title>
    <style>
      :root {{ color-scheme: dark; font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: #0e1215; color: #f1f5f4; }}
      * {{ box-sizing: border-box; }}
      body {{ min-height: 100vh; margin: 0; display: grid; place-items: center; padding: 32px 24px; }}
      main {{ width: min(100%, 620px); }}
      .brand {{ display: flex; align-items: center; gap: 11px; color: #f1f5f4; font-size: 15px; font-weight: 600; }}
      .brand-mark {{ width: 34px; height: 34px; padding: 6px; border: 1px solid #46665f; background: #164a42; }}
      .eyebrow {{ margin: 54px 0 13px; color: #8eaaa4; font-size: 12px; font-weight: 600; letter-spacing: 0; text-transform: uppercase; }}
      h1 {{ max-width: 13ch; margin: 0; font-size: 42px; font-weight: 600; line-height: 1.08; letter-spacing: 0; }}
      .message {{ max-width: 48ch; margin: 18px 0 0; color: #b3c1bf; font-size: 16px; line-height: 1.65; }}
      .status {{ display: flex; align-items: center; gap: 9px; margin-top: 36px; padding-top: 18px; border-top: 1px solid #2b3535; color: #81918f; font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace; font-size: 12px; }}
      .status::before {{ width: 7px; height: 7px; border-radius: 50%; background: #e2ae5d; content: ""; }}
      @media (max-width: 480px) {{ body {{ padding: 28px 20px; }} .eyebrow {{ margin-top: 42px; }} h1 {{ font-size: 34px; }} .message {{ font-size: 15px; }} }}
    </style>
  </head>
  <body>
    <main>
      <div class="brand">
        <img class="brand-mark" src="/ignitify-mark.svg" alt="Ignitify">
        <span>Ignitify</span>
      </div>
      <p class="eyebrow">Ingress response</p>
      <h1>{heading}</h1>
      <p class="message">{message}</p>
      <p class="status">HTTP 404 · No active route</p>
    </main>
  </body>
</html>
"#,
    )
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn set_permissions(path: &Path, mode: u32) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}

impl Ingress for TraefikIngress {
    fn route(
        &self,
        _service_id: &ServiceId,
        domain_id: &DomainId,
        hostname: &DomainName,
        port: u32,
    ) -> ControlResult<IngressRoute> {
        let policy = *self
            .routing_policy
            .read()
            .map_err(|_| ControlError::Runtime)?;
        render_route_with_policy(domain_id, hostname, port, policy)
            .map_err(|_| ControlError::Runtime)
    }

    async fn reconcile(&self) -> ControlResult<()> {
        if let Some(source) = &self.server_settings {
            self.sync_server_settings(source).await?;
        }
        Ok(())
    }

    async fn ensure_ready(&self) -> ControlResult<bool> {
        Ok(self.ensure_started().await)
    }
}

impl RuntimeHealth for TraefikIngress {
    fn ready(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send + '_>> {
        Box::pin(async move { self.ready().await })
    }
}

#[cfg(test)]
mod tests {
    use ignitify_domain::{DomainId, DomainName};

    use super::{
        CERT_RESOLVER, CONTROL_PLANE_ROUTE_FILE, ENTRYPOINT, PROXY_NETWORK, RoutingPolicy,
        TLS_REDIRECT_MIDDLEWARE, clear_dynamic_certificates, fallback_page_path_from_dynamic_dir,
        render_control_plane_route, render_fallback_page, render_route, render_route_with_policy,
        sync_control_plane_route, write_dynamic_certificates, write_fallback_page,
    };

    fn ids() -> (DomainId, DomainName) {
        (
            DomainId::new("00000000-0000-4000-8000-000000000001").unwrap(),
            DomainName::new("app.example.com").unwrap(),
        )
    }

    #[test]
    fn default_route_keeps_platform_tls_and_redirects_http() {
        let (domain_id, hostname) = ids();
        let route = render_route(&domain_id, &hostname, 8080).unwrap();
        let router = "traefik.http.routers.ignitify-00000000-0000-4000-8000-000000000001";
        assert_eq!(route.network, PROXY_NETWORK);
        assert_eq!(route.labels[&format!("{router}.entrypoints")], ENTRYPOINT);
        assert_eq!(
            route.labels[&format!("{router}.tls.certresolver")],
            CERT_RESOLVER
        );
        assert_eq!(
            route.labels[&format!("{router}-http.middlewares")],
            TLS_REDIRECT_MIDDLEWARE
        );
    }

    #[test]
    fn http_policy_omits_tls_labels() {
        let (domain_id, hostname) = ids();
        let route =
            render_route_with_policy(&domain_id, &hostname, 8080, RoutingPolicy::Http).unwrap();
        assert_eq!(
            route.labels["traefik.http.routers.ignitify-00000000-0000-4000-8000-000000000001.entrypoints"],
            "web"
        );
        assert!(route.labels.keys().all(|key| !key.contains(".tls")));
        assert!(route.labels.keys().all(|key| !key.ends_with("-http.rule")));
    }

    #[test]
    fn custom_tls_policy_does_not_request_acme_certificates() {
        let (domain_id, hostname) = ids();
        let route = render_route_with_policy(
            &domain_id,
            &hostname,
            8080,
            RoutingPolicy::Tls {
                certificate_resolver: None,
            },
        )
        .unwrap();
        assert!(route.labels.contains_key(
            "traefik.http.routers.ignitify-00000000-0000-4000-8000-000000000001.tls"
        ));
        assert!(!route.labels.contains_key(
            "traefik.http.routers.ignitify-00000000-0000-4000-8000-000000000001.tls.certresolver"
        ));
    }

    #[test]
    fn dynamic_certificate_config_contains_only_container_paths() {
        let directory =
            std::env::temp_dir().join(format!("ignitify-traefik-{}", uuid::Uuid::new_v4()));
        let certificate_id = "00000000-0000-4000-8000-000000000002";
        write_dynamic_certificates(
            &directory,
            certificate_id,
            b"-----BEGIN CERTIFICATE-----\nexample\n-----END CERTIFICATE-----\n",
            b"-----BEGIN PRIVATE KEY-----\nexample\n-----END PRIVATE KEY-----\n",
        )
        .unwrap();

        let config = std::fs::read_to_string(directory.join("certificates.yml")).unwrap();
        assert!(
            config.contains("/etc/traefik/dynamic/certs/00000000-0000-4000-8000-000000000002.crt")
        );
        assert!(!config.contains("BEGIN CERTIFICATE"));
        assert_eq!(
            std::fs::read(
                directory
                    .join("certs")
                    .join(format!("{certificate_id}.key"))
            )
            .unwrap(),
            b"-----BEGIN PRIVATE KEY-----\nexample\n-----END PRIVATE KEY-----\n"
        );

        clear_dynamic_certificates(&directory).unwrap();
        assert!(!directory.join("certificates.yml").exists());
        assert!(
            !directory
                .join("certs")
                .join(format!("{certificate_id}.crt"))
                .exists()
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn route_rejects_invalid_port() {
        let (domain_id, hostname) = ids();
        assert!(render_route(&domain_id, &hostname, 0).is_err());
    }

    #[test]
    fn fallback_page_escapes_configured_content() {
        let page = render_fallback_page(
            "Application <missing>",
            "Use <support@example.com>\nNext line",
        );

        assert!(page.contains("Application &lt;missing&gt;"));
        assert!(page.contains("Use &lt;support@example.com&gt;<br>Next line"));
        assert!(page.contains("src=\"/ignitify-mark.svg\""));
        assert!(!page.contains("<support@example.com>"));
    }

    #[test]
    fn fallback_page_path_is_next_to_the_dynamic_directory() {
        let path = fallback_page_path_from_dynamic_dir(std::path::Path::new(
            "/var/lib/ignitify/traefik/dynamic",
        ));

        assert_eq!(
            path,
            std::path::Path::new("/var/lib/ignitify/traefik/fallback/404.html")
        );
    }

    #[test]
    fn fallback_page_is_written_to_a_caddy_readable_file() {
        let directory =
            std::env::temp_dir().join(format!("ignitify-traefik-{}", uuid::Uuid::new_v4()));
        let path = directory.join("fallback").join("404.html");

        write_fallback_page(&path, &server_settings()).unwrap();

        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("Application not found")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o644
            );
            assert_eq!(
                std::fs::metadata(path.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o755
            );
        }

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn control_plane_route_uses_the_host_gateway_and_tls() {
        let hostname = DomainName::new("console.example.com").unwrap();
        let config = render_control_plane_route(&hostname, Some(CERT_RESOLVER));

        assert!(config.contains("Host(`console.example.com`)"));
        assert!(config.contains("url: \"http://host.docker.internal:5656\""));
        assert!(config.contains("certResolver: le"));
        assert!(config.contains("redirect-to-https@file"));
        assert_eq!(config.matches("priority: 1000").count(), 2);
    }

    #[test]
    fn clearing_the_control_plane_domain_removes_its_dynamic_route() {
        let directory =
            std::env::temp_dir().join(format!("ignitify-traefik-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(CONTROL_PLANE_ROUTE_FILE);
        std::fs::write(&path, "stale route").unwrap();
        let settings = server_settings();

        sync_control_plane_route(&directory, &settings).unwrap();
        assert!(!path.exists());
        std::fs::remove_dir_all(directory).unwrap();
    }

    fn server_settings() -> ignitify_db::ServerSettingsRecord {
        ignitify_db::ServerSettingsRecord {
            control_plane_domain: String::new(),
            application_domain_suffix: "apps.example.com".to_owned(),
            https_enabled: true,
            automatically_provision_ssl: true,
            acme_email: "ops@example.com".to_owned(),
            dns_record_type: "a".to_owned(),
            dns_record_target: "203.0.113.10".to_owned(),
            fallback_page_heading: "Application not found".to_owned(),
            fallback_page_message: "Try another hostname.".to_owned(),
            certificate_provider: "lets-encrypt".to_owned(),
            custom_certificate_id: None,
            concurrent_builds: 2,
            updated_at: String::new(),
        }
    }
}
