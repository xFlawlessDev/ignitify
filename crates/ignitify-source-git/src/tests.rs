use super::{
    BuildError, BuildLimiter, is_local_image_id, relative_path, source_build_error,
    static_dockerfile,
};
use crate::source_spec::{
    AUTO_EXPOSED_SERVICE, SOURCE_PLACEHOLDER_IMAGE, compose_runtime_spec, first_compose_service,
    is_git_revision, shell_quote,
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
fn git_compose_preserves_an_explicit_exposed_service() {
    let configured = ServiceSpec::compose(
            "services:\n  web:\n    image: nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
            "web",
            Some(8080),
        )
        .unwrap();
    let runtime = compose_runtime_spec(
            &configured,
            "services:\n  app:\n    image: caddy@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n"
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
fn git_compose_auto_detects_the_first_service() {
    let configured = ServiceSpec::compose(
        format!("services:\n  {AUTO_EXPOSED_SERVICE}:\n    image: {SOURCE_PLACEHOLDER_IMAGE}\n"),
        AUTO_EXPOSED_SERVICE,
        Some(8080),
    )
    .unwrap();
    let runtime = compose_runtime_spec(
            &configured,
            "services:\n  app:\n    image: caddy@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n"
                .to_owned(),
        )
        .unwrap();
    let ServiceSpec::Compose {
        exposed_service, ..
    } = runtime
    else {
        panic!("expected Compose runtime specification");
    };
    assert_eq!(exposed_service, "app");
}

#[test]
fn git_compose_source_requires_at_least_one_service() {
    assert!(matches!(
        first_compose_service("services: {}\n"),
        Err(BuildError::InvalidComposeSource)
    ));
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

#[test]
fn missing_railpack_explains_the_control_plane_prerequisite() {
    assert_eq!(
        source_build_error(BuildError::CommandUnavailable("railpack prepare")).to_string(),
        "source build failed: Railpack CLI is not installed on the control-plane host. Install it or set IGNITIFY_RAILPACK_BIN to its absolute path."
    );
}
