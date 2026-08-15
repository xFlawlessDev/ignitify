use ignitify_control_plane::{IngressRoute, RuntimeDeployment};
use ignitify_domain::ServiceSpec;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{Error, Result};

pub(crate) const PROXY_NETWORK: &str = "ignitify-proxy";
pub(crate) const MANAGED_LABEL: &str = "com.ignitify.managed";
pub(crate) const SERVICE_LABEL: &str = "com.ignitify.service-id";
pub(crate) const GENERATION_LABEL: &str = "com.ignitify.generation";

pub(crate) fn render_override(
    deployment: &RuntimeDeployment,
    routes: &[IngressRoute],
    volumes: Vec<(String, String)>,
) -> Result<String> {
    let ServiceSpec::Compose {
        exposed_service, ..
    } = &deployment.spec
    else {
        return Err(Error::UnsupportedSpec);
    };
    let mut labels = vec![
        format!("      {MANAGED_LABEL}: \"true\""),
        format!("      {SERVICE_LABEL}: \"{}\"", deployment.service_id),
        format!("      {GENERATION_LABEL}: \"{}\"", deployment.generation),
    ];
    for route in routes {
        for (key, value) in &route.labels {
            labels.push(format!(
                "      {}: \"{}\"",
                yaml_quote(key),
                yaml_quote(value)
            ));
        }
    }
    let (service_network, network_definition) = if routes.is_empty() {
        (String::new(), String::new())
    } else {
        (
            format!("    networks:\n      - {PROXY_NETWORK}\n"),
            format!("networks:\n  {PROXY_NETWORK}:\n    external: true\n"),
        )
    };
    let volumes = (!volumes.is_empty()).then(|| {
        let entries = volumes
            .into_iter()
            .map(|(name, value)| format!("  \"{}\":\n    name: {value}", yaml_quote(&name)))
            .collect::<Vec<_>>()
            .join("\n");
        format!("volumes:\n{entries}\n")
    });
    Ok(format!(
        "services:\n  {exposed_service}:\n    labels:\n{}\n{}{}{}",
        labels.join("\n"),
        service_network,
        network_definition,
        volumes.unwrap_or_default(),
    ))
}

pub(crate) fn canonical_volume_names(
    deployment: &RuntimeDeployment,
    value: &Value,
) -> Vec<(String, String)> {
    let Some(volumes) = value.get("volumes").and_then(Value::as_object) else {
        return Vec::new();
    };
    volumes
        .keys()
        .map(|name| {
            let mut digest = Sha256::new();
            digest.update(name.as_bytes());
            let digest = format!("{:x}", digest.finalize());
            (
                name.clone(),
                format!("ignitify-{}-{}", deployment.service_id, &digest[..24]),
            )
        })
        .collect()
}

fn yaml_quote(value: &str) -> String {
    value.replace('"', "\\\"")
}
