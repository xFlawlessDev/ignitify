use utoipa::openapi::{
    Components,
    security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};
use utoipa::{Modify, OpenApi};

use crate::handlers::{ai, backup_destinations, health, notifications};

#[rustfmt::skip]
macro_rules! protected_get {
    ($name:ident, $path:literal, $tag:literal) => {
        #[allow(dead_code)]
        #[utoipa::path(
            get,
            path = $path,
            tag = $tag,
            security(("bearerAuth" = [])),
            responses((status = 200, description = "Operation response"))
        )]
        fn $name() {}
    };
}

#[rustfmt::skip]
macro_rules! protected_get_param {
    ($name:ident, $path:literal, $tag:literal, $param:literal) => {
        #[allow(dead_code)]
        #[utoipa::path(
            get,
            path = $path,
            tag = $tag,
            params(($param = String, Path, description = "Resource identifier")),
            security(("bearerAuth" = [])),
            responses((status = 200, description = "Operation response"))
        )]
        fn $name() {}
    };
}

#[rustfmt::skip]
macro_rules! protected_mutation {
    ($name:ident, $method:ident, $path:literal, $tag:literal) => {
        #[allow(dead_code)]
        #[utoipa::path(
            $method,
            path = $path,
            tag = $tag,
            params((
                "X-Ignitify-Request" = String,
                Header,
                description = "Required same-origin request marker"
            )),
            security(("bearerAuth" = [])),
            responses((status = 200, description = "Operation response"))
        )]
        fn $name() {}
    };
}

#[rustfmt::skip]
macro_rules! protected_mutation_param {
    ($name:ident, $method:ident, $path:literal, $tag:literal, $param:literal) => {
        #[allow(dead_code)]
        #[utoipa::path(
            $method,
            path = $path,
            tag = $tag,
            params(
                ($param = String, Path, description = "Resource identifier"),
                (
                    "X-Ignitify-Request" = String,
                    Header,
                    description = "Required same-origin request marker"
                )
            ),
            security(("bearerAuth" = [])),
            responses((status = 200, description = "Operation response"))
        )]
        fn $name() {}
    };
}

#[rustfmt::skip]
macro_rules! public_get {
    ($name:ident, $path:literal, $tag:literal) => {
        #[allow(dead_code)]
        #[utoipa::path(
            get,
            path = $path,
            tag = $tag,
            responses((status = 200, description = "Operation response"))
        )]
        fn $name() {}
    };
}

#[rustfmt::skip]
macro_rules! public_mutation {
    ($name:ident, $method:ident, $path:literal, $tag:literal) => {
        #[allow(dead_code)]
        #[utoipa::path(
            $method,
            path = $path,
            tag = $tag,
            params((
                "X-Ignitify-Request" = String,
                Header,
                description = "Required same-origin request marker"
            )),
            responses((status = 200, description = "Operation response"))
        )]
        fn $name() {}
    };
}

#[rustfmt::skip]
macro_rules! agent_mutation {
    ($name:ident, $path:literal, $tag:literal) => {
        #[allow(dead_code)]
        #[utoipa::path(
            post,
            path = $path,
            tag = $tag,
            security(("agentBearerAuth" = [])),
            responses((status = 200, description = "Operation response"))
        )]
        fn $name() {}
    };
}

public_get!(auth_bootstrap_status_doc, "/api/v1/auth/bootstrap", "Auth");
public_mutation!(auth_bootstrap_doc, post, "/api/v1/auth/bootstrap", "Auth");
public_mutation!(auth_login_doc, post, "/api/v1/auth/login", "Auth");
public_mutation!(auth_refresh_doc, post, "/api/v1/auth/refresh", "Auth");
public_mutation!(auth_logout_doc, post, "/api/v1/auth/logout", "Auth");
protected_mutation!(auth_step_up_doc, post, "/api/v1/auth/step-up", "Auth");
protected_get!(auth_me_doc, "/api/v1/auth/me", "Auth");

protected_get!(dashboard_doc, "/api/v1/dashboard", "Dashboard");
protected_get!(
    operations_health_summary_doc,
    "/api/v1/operations/health-summary",
    "Operations"
);

protected_get!(providers_list_doc, "/api/v1/providers", "Providers");
protected_mutation!(providers_create_doc, post, "/api/v1/providers", "Providers");
protected_mutation!(
    providers_github_manifest_doc,
    post,
    "/api/v1/providers/github/manifest",
    "Providers"
);
public_get!(
    providers_github_callback_doc,
    "/api/v1/providers/github/manifest/callback",
    "Providers"
);
protected_mutation_param!(
    providers_remove_doc,
    delete,
    "/api/v1/providers/{provider_id}",
    "Providers",
    "provider_id"
);
protected_mutation_param!(
    providers_update_doc,
    patch,
    "/api/v1/providers/{provider_id}",
    "Providers",
    "provider_id"
);
protected_mutation_param!(
    providers_test_doc,
    post,
    "/api/v1/providers/{provider_id}/test",
    "Providers",
    "provider_id"
);
protected_get_param!(
    providers_repositories_doc,
    "/api/v1/providers/{provider_id}/repositories",
    "Providers",
    "provider_id"
);
protected_get_param!(
    providers_branches_doc,
    "/api/v1/providers/{provider_id}/branches",
    "Providers",
    "provider_id"
);

protected_get!(runtime_status_doc, "/api/v1/runtime/status", "Runtime");
protected_get!(
    runtime_containers_doc,
    "/api/v1/runtime/containers",
    "Runtime"
);
protected_get_param!(
    runtime_container_details_doc,
    "/api/v1/runtime/containers/{container_id}/details",
    "Runtime",
    "container_id"
);
protected_get_param!(
    runtime_container_logs_doc,
    "/api/v1/runtime/containers/{container_id}/logs",
    "Runtime",
    "container_id"
);
protected_mutation_param!(
    runtime_upload_container_file_doc,
    post,
    "/api/v1/runtime/containers/{container_id}/upload",
    "Runtime",
    "container_id"
);
protected_get_param!(
    runtime_container_terminal_doc,
    "/api/v1/runtime/containers/{container_id}/terminal",
    "Runtime",
    "container_id"
);
protected_mutation_param!(
    runtime_remove_container_doc,
    delete,
    "/api/v1/runtime/containers/{container_id}",
    "Runtime",
    "container_id"
);
protected_get!(runtime_metrics_doc, "/api/v1/runtime/metrics", "Runtime");

// AI routes describe concrete request bodies because their payloads are user-authored
// conversations rather than generic control-plane mutations.

protected_get!(
    infrastructure_settings_doc,
    "/api/v1/settings/infrastructure",
    "Settings"
);
protected_mutation!(
    update_infrastructure_settings_doc,
    patch,
    "/api/v1/settings/infrastructure",
    "Settings"
);
protected_get!(
    supply_chain_policy_doc,
    "/api/v1/settings/supply-chain-policy",
    "Settings"
);
protected_mutation!(
    update_supply_chain_policy_doc,
    put,
    "/api/v1/settings/supply-chain-policy",
    "Settings"
);
protected_mutation!(
    create_infrastructure_certificate_doc,
    post,
    "/api/v1/settings/infrastructure/certificates",
    "Settings"
);
protected_mutation_param!(
    remove_infrastructure_certificate_doc,
    delete,
    "/api/v1/settings/infrastructure/certificates/{certificate_id}",
    "Settings",
    "certificate_id"
);
protected_get!(server_settings_doc, "/api/v1/settings/server", "Settings");
protected_mutation!(
    update_server_settings_doc,
    patch,
    "/api/v1/settings/server",
    "Settings"
);
protected_mutation!(
    create_server_certificate_doc,
    post,
    "/api/v1/settings/server/certificates",
    "Settings"
);
protected_mutation_param!(
    remove_server_certificate_doc,
    delete,
    "/api/v1/settings/server/certificates/{certificate_id}",
    "Settings",
    "certificate_id"
);

protected_get!(
    remote_builders_doc,
    "/api/v1/remote-builders",
    "Remote Builders"
);
protected_mutation!(
    create_remote_builder_doc,
    post,
    "/api/v1/remote-builders",
    "Remote Builders"
);
protected_mutation_param!(
    remove_remote_builder_doc,
    delete,
    "/api/v1/remote-builders/{builder_id}",
    "Remote Builders",
    "builder_id"
);
protected_mutation_param!(
    update_remote_builder_doc,
    patch,
    "/api/v1/remote-builders/{builder_id}",
    "Remote Builders",
    "builder_id"
);
protected_mutation_param!(
    default_remote_builder_doc,
    post,
    "/api/v1/remote-builders/{builder_id}/default",
    "Remote Builders",
    "builder_id"
);

protected_get!(
    remote_servers_doc,
    "/api/v1/remote-servers",
    "Remote Servers"
);
protected_mutation!(
    create_remote_server_doc,
    post,
    "/api/v1/remote-servers",
    "Remote Servers"
);
protected_mutation_param!(
    remove_remote_server_doc,
    delete,
    "/api/v1/remote-servers/{server_id}",
    "Remote Servers",
    "server_id"
);
protected_mutation_param!(
    update_remote_server_doc,
    patch,
    "/api/v1/remote-servers/{server_id}",
    "Remote Servers",
    "server_id"
);
protected_mutation_param!(
    default_remote_server_doc,
    post,
    "/api/v1/remote-servers/{server_id}/default",
    "Remote Servers",
    "server_id"
);
protected_mutation_param!(
    check_remote_server_doc,
    post,
    "/api/v1/remote-servers/{server_id}/check",
    "Remote Servers",
    "server_id"
);
protected_get_param!(
    remote_server_access_doc,
    "/api/v1/remote-servers/{server_id}/access",
    "Remote Servers",
    "server_id"
);
protected_get_param!(
    remote_server_agent_doc,
    "/api/v1/remote-servers/{server_id}/agent",
    "Remote Servers",
    "server_id"
);
protected_mutation_param!(
    install_remote_server_agent_doc,
    post,
    "/api/v1/remote-servers/{server_id}/agent/install",
    "Remote Servers",
    "server_id"
);
agent_mutation!(
    remote_agent_heartbeat_doc,
    "/api/v1/remote-agents/heartbeat",
    "Remote Servers"
);

protected_get!(uptime_monitors_doc, "/api/v1/uptime-monitors", "Monitoring");
protected_mutation!(
    create_uptime_monitor_doc,
    post,
    "/api/v1/uptime-monitors",
    "Monitoring"
);
protected_mutation_param!(
    remove_uptime_monitor_doc,
    delete,
    "/api/v1/uptime-monitors/{monitor_id}",
    "Monitoring",
    "monitor_id"
);
protected_mutation_param!(
    update_uptime_monitor_doc,
    patch,
    "/api/v1/uptime-monitors/{monitor_id}",
    "Monitoring",
    "monitor_id"
);
protected_get_param!(
    uptime_monitor_history_doc,
    "/api/v1/uptime-monitors/{monitor_id}/history",
    "Monitoring",
    "monitor_id"
);

protected_get!(projects_doc, "/api/v1/projects", "Projects");
protected_mutation!(create_project_doc, post, "/api/v1/projects", "Projects");
protected_get_param!(
    get_project_doc,
    "/api/v1/projects/{project_id}",
    "Projects",
    "project_id"
);
protected_mutation_param!(
    update_project_doc,
    patch,
    "/api/v1/projects/{project_id}",
    "Projects",
    "project_id"
);
protected_mutation_param!(
    remove_project_doc,
    delete,
    "/api/v1/projects/{project_id}",
    "Projects",
    "project_id"
);
protected_get_param!(
    get_project_environment_doc,
    "/api/v1/projects/{project_id}/environment",
    "Projects",
    "project_id"
);
protected_mutation_param!(
    update_project_environment_doc,
    put,
    "/api/v1/projects/{project_id}/environment",
    "Projects",
    "project_id"
);
protected_get_param!(
    project_deployments_doc,
    "/api/v1/projects/{project_id}/deployments",
    "Projects",
    "project_id"
);
protected_get_param!(
    project_activity_doc,
    "/api/v1/projects/{project_id}/activity",
    "Projects",
    "project_id"
);
protected_get_param!(
    project_services_doc,
    "/api/v1/projects/{project_id}/services",
    "Services",
    "project_id"
);
protected_mutation_param!(
    create_project_service_doc,
    post,
    "/api/v1/projects/{project_id}/services",
    "Services",
    "project_id"
);

protected_get_param!(
    get_service_doc,
    "/api/v1/services/{service_id}",
    "Services",
    "service_id"
);
protected_mutation_param!(
    update_service_doc,
    patch,
    "/api/v1/services/{service_id}",
    "Services",
    "service_id"
);
protected_mutation_param!(
    remove_service_doc,
    delete,
    "/api/v1/services/{service_id}",
    "Services",
    "service_id"
);
protected_mutation_param!(
    rotate_service_auto_deploy_secret_doc,
    post,
    "/api/v1/services/{service_id}/auto-deploy-secret",
    "Services",
    "service_id"
);
protected_get_param!(
    service_deployments_doc,
    "/api/v1/services/{service_id}/deployments",
    "Deployments",
    "service_id"
);
protected_mutation_param!(
    deploy_service_doc,
    post,
    "/api/v1/services/{service_id}/deployments",
    "Deployments",
    "service_id"
);
protected_get_param!(
    service_domains_doc,
    "/api/v1/services/{service_id}/domains",
    "Domains",
    "service_id"
);
protected_mutation_param!(
    create_service_domain_doc,
    post,
    "/api/v1/services/{service_id}/domains",
    "Domains",
    "service_id"
);
protected_mutation_param!(
    stop_service_doc,
    post,
    "/api/v1/services/{service_id}/stop",
    "Services",
    "service_id"
);

#[allow(dead_code)]
#[utoipa::path(
    post,
    path = "/api/v1/webhooks/services/{service_id}",
    tag = "Webhooks",
    params(
        ("service_id" = String, Path, description = "Service identifier"),
        ("X-GitHub-Event" = Option<String>, Header, description = "GitHub event name"),
        ("X-Hub-Signature-256" = Option<String>, Header, description = "GitHub HMAC SHA-256 signature"),
        ("X-GitLab-Event" = Option<String>, Header, description = "GitLab event name"),
        ("X-GitLab-Token" = Option<String>, Header, description = "GitLab webhook secret token"),
        ("X-Gitea-Event" = Option<String>, Header, description = "Gitea event name"),
        ("X-Gitea-Signature" = Option<String>, Header, description = "Gitea HMAC SHA-256 signature")
    ),
    request_body = String,
    responses(
        (status = 204, description = "Event ignored or deployment queued"),
        (status = 401, description = "Webhook signature or token was invalid")
    )
)]
fn auto_deploy_webhook_doc() {}

protected_get_param!(
    get_deployment_doc,
    "/api/v1/deployments/{deployment_id}",
    "Deployments",
    "deployment_id"
);
protected_get_param!(
    deployment_events_doc,
    "/api/v1/deployments/{deployment_id}/events",
    "Deployments",
    "deployment_id"
);
protected_get_param!(
    deployment_logs_doc,
    "/api/v1/deployments/{deployment_id}/logs",
    "Deployments",
    "deployment_id"
);
protected_mutation_param!(
    rollback_deployment_doc,
    post,
    "/api/v1/deployments/{deployment_id}/rollback",
    "Deployments",
    "deployment_id"
);
protected_mutation_param!(
    approve_deployment_doc,
    post,
    "/api/v1/deployments/{deployment_id}/approve",
    "Deployments",
    "deployment_id"
);
protected_mutation_param!(
    cancel_deployment_doc,
    post,
    "/api/v1/deployments/{deployment_id}/cancel",
    "Deployments",
    "deployment_id"
);

protected_mutation_param!(
    remove_domain_doc,
    delete,
    "/api/v1/domains/{domain_id}",
    "Domains",
    "domain_id"
);
protected_mutation_param!(
    verify_domain_doc,
    post,
    "/api/v1/domains/{domain_id}/verify",
    "Domains",
    "domain_id"
);

protected_get!(terminal_doc, "/api/v1/terminal", "Terminal");

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Ignitify API",
        version = env!("CARGO_PKG_VERSION"),
        description = "Authenticated control-plane API for self-hosted Ignitify installations."
    ),
    paths(
        health::health,
        auth_bootstrap_status_doc,
        auth_bootstrap_doc,
        auth_login_doc,
        auth_refresh_doc,
        auth_logout_doc,
        auth_step_up_doc,
        auth_me_doc,
        dashboard_doc,
        operations_health_summary_doc,
        notifications::list,
        notifications::list_deliveries,
        notifications::create,
        notifications::update,
        notifications::remove,
        providers_list_doc,
        providers_create_doc,
        providers_github_manifest_doc,
        providers_github_callback_doc,
        providers_remove_doc,
        providers_update_doc,
        providers_test_doc,
        providers_repositories_doc,
        providers_branches_doc,
        runtime_status_doc,
        runtime_containers_doc,
        runtime_container_details_doc,
        runtime_container_logs_doc,
        runtime_upload_container_file_doc,
        runtime_container_terminal_doc,
        runtime_remove_container_doc,
        runtime_metrics_doc,
        ai::get_settings,
        ai::update_settings,
        ai::chat,
        infrastructure_settings_doc,
        update_infrastructure_settings_doc,
        supply_chain_policy_doc,
        update_supply_chain_policy_doc,
        create_infrastructure_certificate_doc,
        remove_infrastructure_certificate_doc,
        server_settings_doc,
        update_server_settings_doc,
        create_server_certificate_doc,
        remove_server_certificate_doc,
        backup_destinations::get,
        backup_destinations::upsert,
        backup_destinations::remove,
        backup_destinations::update_controls,
        backup_destinations::list_runs,
        remote_builders_doc,
        create_remote_builder_doc,
        remove_remote_builder_doc,
        update_remote_builder_doc,
        default_remote_builder_doc,
        remote_servers_doc,
        create_remote_server_doc,
        remove_remote_server_doc,
        update_remote_server_doc,
        default_remote_server_doc,
        check_remote_server_doc,
        remote_server_agent_doc,
        install_remote_server_agent_doc,
        remote_agent_heartbeat_doc,
        uptime_monitors_doc,
        create_uptime_monitor_doc,
        remove_uptime_monitor_doc,
        update_uptime_monitor_doc,
        uptime_monitor_history_doc,
        projects_doc,
        create_project_doc,
        get_project_doc,
        update_project_doc,
        remove_project_doc,
        get_project_environment_doc,
        update_project_environment_doc,
        project_deployments_doc,
        project_activity_doc,
        project_services_doc,
        create_project_service_doc,
        get_service_doc,
        update_service_doc,
        remove_service_doc,
        rotate_service_auto_deploy_secret_doc,
        service_deployments_doc,
        deploy_service_doc,
        service_domains_doc,
        create_service_domain_doc,
        stop_service_doc,
        auto_deploy_webhook_doc,
        get_deployment_doc,
        deployment_events_doc,
        deployment_logs_doc,
        rollback_deployment_doc,
        approve_deployment_doc,
        cancel_deployment_doc,
        remove_domain_doc,
        verify_domain_doc,
        terminal_doc
    ),
    components(
        schemas(
            health::HealthResponse,
            ai::AiSettingsRequest,
            ai::AiSettingsResponse,
            ai::AiChatRequest,
            ai::AiChatMessageRequest,
            ai::AiLogContextRequest,
            ai::AiChatResponse,
            backup_destinations::BackupS3DestinationRequest,
            backup_destinations::BackupS3DestinationResponse,
            backup_destinations::BackupS3ControlsRequest,
            backup_destinations::BackupS3RunResponse,
            notifications::NotificationChannelRequest,
            notifications::NotificationChannelResponse,
            notifications::NotificationDeliveryQuery,
            notifications::NotificationDeliveryResponse
        )
    ),
    tags(
        (name = "Health", description = "Runtime readiness checks"),
        (name = "Auth", description = "Authentication and session lifecycle"),
        (name = "Dashboard", description = "Dashboard aggregates"),
        (name = "Operations", description = "Operator health and operational metrics"),
        (name = "Notifications", description = "Operator notification channel management"),
        (name = "Providers", description = "Source-control provider connections"),
        (name = "AI", description = "OpenAI-compatible operations assistant configuration and chat"),
        (name = "Settings", description = "Infrastructure and server settings"),
        (name = "Backup", description = "S3 destination, scheduler, and run history"),
        (name = "Remote Builders", description = "Remote build worker management"),
        (name = "Remote Servers", description = "Remote server and agent management"),
        (name = "Runtime", description = "Containers and runtime telemetry"),
        (name = "Monitoring", description = "Uptime monitor management"),
        (name = "Projects", description = "Project and environment management"),
        (name = "Services", description = "Service configuration and lifecycle"),
        (name = "Deployments", description = "Deployment history and controls"),
        (name = "Domains", description = "Service domain management"),
        (name = "Terminal", description = "Interactive terminal connections")
        ,(name = "Webhooks", description = "Verified source-control push event delivery")
    )
)]
struct ApiDoc;

struct BearerSecurity;

impl Modify for BearerSecurity {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Components::new);
        components.add_security_scheme(
            "bearerAuth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        );
    }
}

struct AgentBearerSecurity;

impl Modify for AgentBearerSecurity {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Components::new);
        components.add_security_scheme(
            "agentBearerAuth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("Remote agent token")
                    .build(),
            ),
        );
    }
}

pub(crate) fn document() -> utoipa::openapi::OpenApi {
    let mut document = ApiDoc::openapi();
    BearerSecurity.modify(&mut document);
    AgentBearerSecurity.modify(&mut document);
    document
}
