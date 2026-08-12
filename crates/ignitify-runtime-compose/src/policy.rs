use std::collections::{BTreeSet, HashSet};

use serde_json::Value;
use yaml_rust2::{
    Yaml, YamlLoader,
    parser::{Event, EventReceiver, Parser},
};

use crate::{Error, Result};

const MAX_YAML_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_DEPTH: usize = 64;
const MAX_SERVICES: usize = 100;

/// Performs host-independent Compose validation before desired configuration is persisted.
pub fn validate_submission_yaml(source: &str) -> std::result::Result<(), &'static str> {
    preflight_yaml(source).map_err(|error| match error {
        Error::Policy(message) => message,
        _ => "invalid YAML",
    })
}

pub(crate) fn preflight_yaml(source: &str) -> Result<()> {
    if source.len() > MAX_YAML_BYTES {
        return Err(Error::Policy("YAML exceeds 1 MiB"));
    }
    reject_yaml_mechanisms(source)?;
    let documents = YamlLoader::load_from_str(source).map_err(|_| Error::Policy("invalid YAML"))?;
    if documents.len() != 1 {
        return Err(Error::Policy("exactly one YAML document required"));
    }
    let mut services = 0;
    inspect_yaml(&documents[0], 0, &mut services)?;
    if services == 0 {
        return Err(Error::Policy("Compose services are required"));
    }
    Ok(())
}

fn reject_yaml_mechanisms(source: &str) -> Result<()> {
    enum Context {
        Mapping {
            keys: HashSet<String>,
            expecting_key: bool,
        },
        Sequence,
    }

    struct Sink {
        contexts: Vec<Context>,
        rejected: bool,
    }

    impl EventReceiver for Sink {
        fn on_event(&mut self, event: Event) {
            match event {
                Event::Alias(_) => self.rejected = true,
                Event::Scalar(value, _, anchor, tag) => {
                    if anchor != 0 || tag.is_some() {
                        self.rejected = true;
                    }
                    if let Some(Context::Mapping {
                        keys,
                        expecting_key,
                    }) = self.contexts.last_mut()
                    {
                        if *expecting_key {
                            if !keys.insert(value) {
                                self.rejected = true;
                            }
                            *expecting_key = false;
                        } else {
                            *expecting_key = true;
                        }
                    }
                }
                Event::MappingStart(anchor, tag) => {
                    if anchor != 0 || tag.is_some() {
                        self.rejected = true;
                    }
                    mark_value_complete(&mut self.contexts);
                    self.contexts.push(Context::Mapping {
                        keys: HashSet::new(),
                        expecting_key: true,
                    });
                }
                Event::SequenceStart(anchor, tag) => {
                    if anchor != 0 || tag.is_some() {
                        self.rejected = true;
                    }
                    mark_value_complete(&mut self.contexts);
                    self.contexts.push(Context::Sequence);
                }
                Event::MappingEnd | Event::SequenceEnd => {
                    self.contexts.pop();
                    mark_value_complete(&mut self.contexts);
                }
                Event::Nothing
                | Event::StreamStart
                | Event::StreamEnd
                | Event::DocumentStart
                | Event::DocumentEnd => {}
            }
        }
    }

    fn mark_value_complete(contexts: &mut [Context]) {
        if let Some(Context::Mapping { expecting_key, .. }) = contexts.last_mut() {
            *expecting_key = true;
        }
    }

    let mut sink = Sink {
        contexts: Vec::new(),
        rejected: false,
    };
    Parser::new_from_str(source)
        .load(&mut sink, true)
        .map_err(|_| Error::Policy("invalid YAML"))?;
    if sink.rejected {
        Err(Error::Policy(
            "YAML aliases, anchors, tags, and duplicate keys are forbidden",
        ))
    } else {
        Ok(())
    }
}

fn inspect_yaml(value: &Yaml, depth: usize, services: &mut usize) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(Error::Policy("YAML nesting exceeds 64 levels"));
    }
    match value {
        Yaml::Alias(_) => return Err(Error::Policy("YAML aliases are forbidden")),
        Yaml::Hash(map) => {
            let mut keys = BTreeSet::new();
            for (key, child) in map {
                let key = match key {
                    Yaml::String(key) => key,
                    _ => return Err(Error::Policy("mapping keys must be strings")),
                };
                if !keys.insert(key) {
                    return Err(Error::Policy("duplicate YAML key"));
                }
                if key == "services"
                    && let Yaml::Hash(service_map) = child
                {
                    *services = service_map.len();
                    if *services > MAX_SERVICES {
                        return Err(Error::Policy("too many services"));
                    }
                }
                if let Some(reason) = forbidden_compose_field(key) {
                    return Err(Error::Policy(reason));
                }
                if key == "image"
                    && !matches!(child, Yaml::String(image) if is_compose_image(image))
                {
                    return Err(Error::Policy(
                        "every Compose image must use a SHA-256 digest",
                    ));
                }
                if key == "volumes"
                    && let Yaml::Hash(volumes) = child
                    && volumes.values().any(|volume| {
                        matches!(volume, Yaml::Hash(options) if options.keys().any(|option| {
                            matches!(option, Yaml::String(name) if matches!(name.as_str(), "external" | "name" | "driver" | "driver_opts"))
                        }))
                    })
                {
                    return Err(Error::Policy(
                        "external, named, or configured volumes are forbidden",
                    ));
                }
                if key == "volumes"
                    && let Yaml::Array(volumes) = child
                    && volumes.iter().any(|volume| {
                        matches!(volume, Yaml::String(value) if value.split_once(':').map_or(value.as_str(), |(source, _)| source).contains('/') || value.split_once(':').map_or(value.as_str(), |(source, _)| source).starts_with('.'))
                    })
                {
                    return Err(Error::Policy("bind mounts are forbidden"));
                }
                inspect_yaml(child, depth + 1, services)?;
            }
        }
        Yaml::Array(items) => {
            for child in items {
                inspect_yaml(child, depth + 1, services)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn validate_canonical(value: &Value, generated: bool) -> Result<()> {
    let services = value
        .get("services")
        .and_then(Value::as_object)
        .ok_or(Error::Policy("services must be an object"))?;
    if services.is_empty() || services.len() > MAX_SERVICES {
        return Err(Error::Policy("invalid service count"));
    }
    validate_json_keys(value, generated, &[])?;
    if let Some(volumes) = value.get("volumes").and_then(Value::as_object) {
        for volume in volumes.values() {
            let Some(volume) = volume.as_object() else {
                continue;
            };
            if volume.get("external").and_then(Value::as_bool) == Some(true)
                || volume.contains_key("driver")
                || volume.contains_key("driver_opts")
            {
                return Err(Error::Policy(
                    "only unnamed local Compose volumes are allowed",
                ));
            }
        }
    }
    for service in services.values() {
        let service = service
            .as_object()
            .ok_or(Error::Policy("service must be an object"))?;
        const ALLOWED_SERVICE_KEYS: &[&str] = &[
            "image",
            "command",
            "entrypoint",
            "environment",
            "volumes",
            "networks",
            "depends_on",
            "restart",
            "healthcheck",
            "working_dir",
            "user",
            "stdin_open",
            "tty",
            "read_only",
            "init",
            "stop_signal",
            "stop_grace_period",
            "expose",
            "platform",
        ];
        if service.keys().any(|key| {
            !(ALLOWED_SERVICE_KEYS.contains(&key.as_str()) || generated && key == "labels")
        }) {
            return Err(Error::Policy("unsupported Compose service field"));
        }
        if let Some(volumes) = service.get("volumes").and_then(Value::as_array) {
            for volume in volumes {
                match volume {
                    Value::String(value) => {
                        let source = value
                            .split_once(':')
                            .map_or(value.as_str(), |(source, _)| source);
                        if source.contains('/') || source.starts_with('.') || source.is_empty() {
                            return Err(Error::Policy("bind mounts are forbidden"));
                        }
                    }
                    Value::Object(value) => {
                        if value.get("type").and_then(Value::as_str) != Some("volume")
                            || value.get("source").and_then(Value::as_str).is_none()
                        {
                            return Err(Error::Policy("only named local volumes are allowed"));
                        }
                    }
                    _ => return Err(Error::Policy("invalid Compose volume")),
                }
            }
        }
        let image = service
            .get("image")
            .and_then(Value::as_str)
            .ok_or(Error::Policy("every service needs image"))?;
        if !is_compose_image(image) {
            return Err(Error::Policy(
                "every Compose image must use a SHA-256 digest",
            ));
        }
    }
    Ok(())
}

fn validate_json_keys(value: &Value, generated: bool, path: &[&str]) -> Result<()> {
    if let Value::Object(map) = value {
        for (key, child) in map {
            let generated_network =
                generated && path == ["networks", "ignitify-proxy"] && key == "external";
            if key == "type" && child.as_str() == Some("bind") {
                return Err(Error::Policy("bind mounts are forbidden"));
            }
            if let Some(reason) = forbidden_canonical_field(key) {
                return Err(Error::Policy(reason));
            }
            if key == "external" && child.as_bool() == Some(true) && !generated_network {
                return Err(Error::Policy("external Compose resources are forbidden"));
            }
            if key.starts_with("traefik.") && !generated {
                return Err(Error::Policy("raw Traefik labels are forbidden"));
            }
            if key == "bind" {
                return Err(Error::Policy("bind mounts are forbidden"));
            }
            let next_path = if path.len() < 2 {
                let mut next_path = path.to_vec();
                next_path.push(key.as_str());
                next_path
            } else {
                path.to_vec()
            };
            validate_json_keys(child, generated, &next_path)?;
        }
    } else if let Value::Array(items) = value {
        for child in items {
            validate_json_keys(child, generated, path)?;
        }
    }
    Ok(())
}

fn forbidden_compose_field(key: &str) -> Option<&'static str> {
    match key {
        "build" => Some("Compose builds are forbidden"),
        "ports" => Some("Compose host ports are forbidden"),
        "network_mode" => Some("Compose network mode is forbidden"),
        "pid" | "ipc" | "uts" => Some("Compose namespace sharing is forbidden"),
        "privileged" => Some("privileged Compose services are forbidden"),
        "cap_add" | "cap_drop" => Some("Compose capability changes are forbidden"),
        "devices" | "gpus" | "runtime" => Some("Compose device access is forbidden"),
        "volumes_from" => Some("Compose shared volumes are forbidden"),
        "security_opt" | "sysctls" => Some("Compose security options are forbidden"),
        "include" | "extends" => Some("Compose file inclusion is forbidden"),
        "profiles" => Some("Compose profiles are forbidden"),
        "env_file" | "label_file" => Some("Compose external files are forbidden"),
        "external" => Some("external Compose resources are forbidden"),
        "driver" | "driver_opts" => Some("configured Compose volume drivers are forbidden"),
        "external_links" => Some("Compose external links are forbidden"),
        "configs" | "secrets" => Some("Compose external resources are forbidden"),
        "<<" => Some("YAML merge keys are forbidden"),
        _ => None,
    }
}

fn forbidden_canonical_field(key: &str) -> Option<&'static str> {
    match key {
        "build" | "ports" | "network_mode" | "pid" | "ipc" | "uts" | "privileged" | "cap_add"
        | "cap_drop" | "devices" | "gpus" | "volumes_from" | "runtime" | "security_opt"
        | "sysctls" | "driver_opts" => forbidden_compose_field(key),
        "external_links" => Some("Compose external links are forbidden"),
        "configs" | "secrets" => Some("Compose external resources are forbidden"),
        "<<" => Some("YAML merge keys are forbidden"),
        _ => None,
    }
}

pub(crate) fn ensure_exposed_service(value: &Value, service: &str) -> Result<()> {
    if value
        .get("services")
        .and_then(Value::as_object)
        .is_some_and(|services| services.contains_key(service))
    {
        Ok(())
    } else {
        Err(Error::Policy("selected exposed service does not exist"))
    }
}

pub(crate) fn is_compose_image(value: &str) -> bool {
    ignitify_domain::is_digest_image_reference(value)
}
