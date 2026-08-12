use std::path::{Path, PathBuf};

use ignitify_db::{ProviderKind, ProviderRecord};
use ignitify_domain::ServiceSpec;
use url::Url;
use yaml_rust2::YamlLoader;

use crate::build_support::BuildError;

pub(crate) const AUTO_EXPOSED_SERVICE: &str = "ignitify";
pub(crate) const SOURCE_PLACEHOLDER_IMAGE: &str = "ignitify-source-placeholder@sha256:0000000000000000000000000000000000000000000000000000000000000000";

pub(crate) fn repository_url(
    provider: &ProviderRecord,
    repository: &str,
) -> Result<String, BuildError> {
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

pub(crate) fn relative_path(value: &str) -> Result<PathBuf, BuildError> {
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

pub(crate) fn compose_runtime_spec(
    deployment_spec: &ServiceSpec,
    yaml: String,
) -> Result<ServiceSpec, BuildError> {
    let ServiceSpec::Compose {
        exposed_service: configured_exposed_service,
        internal_port,
        ..
    } = deployment_spec
    else {
        return Err(BuildError::InvalidComposeSource);
    };
    let exposed_service = if has_auto_exposed_service(deployment_spec) {
        first_compose_service(&yaml)?
    } else {
        configured_exposed_service.to_owned()
    };
    ServiceSpec::compose(yaml, exposed_service, *internal_port)
        .map_err(|_| BuildError::InvalidComposeSource)
}

fn has_auto_exposed_service(spec: &ServiceSpec) -> bool {
    let ServiceSpec::Compose {
        yaml,
        exposed_service,
        ..
    } = spec
    else {
        return false;
    };
    exposed_service == AUTO_EXPOSED_SERVICE
        && yaml
            == &format!(
                "services:\n  {AUTO_EXPOSED_SERVICE}:\n    image: {SOURCE_PLACEHOLDER_IMAGE}\n"
            )
}

pub(crate) fn first_compose_service(yaml: &str) -> Result<String, BuildError> {
    let documents =
        YamlLoader::load_from_str(yaml).map_err(|_| BuildError::InvalidComposeSource)?;
    let services = documents
        .first()
        .and_then(|document| document["services"].as_hash())
        .ok_or(BuildError::InvalidComposeSource)?;
    services
        .keys()
        .find_map(|name| name.as_str())
        .map(str::to_owned)
        .ok_or(BuildError::InvalidComposeSource)
}

pub(crate) fn static_dockerfile(
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

pub(crate) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(crate) fn is_git_revision(value: &str) -> bool {
    (40..=128).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) fn is_local_image_id(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}
