//! Domain models and validation for Ignitify resources.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

mod dns;
mod supply_chain;

pub use dns::{DnsRecord, DnsRecordTarget, DnsRecordType, DnsVerificationStatus};
pub use supply_chain::{
    SupplyChainCheck, SupplyChainCheckStatus, SupplyChainEnforcement, SupplyChainReport,
    evaluate_supply_chain_report,
};

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self> {
                let value = value.into();
                if !is_uuid(&value) {
                    return Err(InputError::InvalidId);
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = InputError;

            fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
                Self::new(value)
            }
        }
    };
}

uuid_id!(DeploymentId);
uuid_id!(DomainId);
uuid_id!(EnvironmentId);
uuid_id!(ProjectId);
uuid_id!(ServiceId);
uuid_id!(UserId);

fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            matches!(index, 8 | 13 | 18 | 23) && byte == b'-'
                || !matches!(index, 8 | 13 | 18 | 23) && byte.is_ascii_hexdigit()
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainName(String);

impl DomainName {
    pub fn new(value: impl AsRef<str>) -> Result<Self> {
        let value = value.as_ref();
        if !is_domain_name(value) {
            return Err(InputError::InvalidDomainName);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DomainName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for DomainName {
    type Err = InputError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainStatus {
    Pending,
    Active,
    Failed,
}

impl DomainStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Failed => "failed",
        }
    }
}

impl TryFrom<&str> for DomainStatus {
    type Error = InputError;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "pending" => Ok(Self::Pending),
            "active" => Ok(Self::Active),
            "failed" => Ok(Self::Failed),
            _ => Err(InputError::InvalidDomainStatus),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInput {
    pub name: String,
}

impl ProjectInput {
    pub fn new(name: impl AsRef<str>) -> Result<Self> {
        let name = name.as_ref().trim();
        if !(1..=100).contains(&name.chars().count()) || name.chars().any(char::is_control) {
            return Err(InputError::InvalidProjectName);
        }
        Ok(Self {
            name: name.to_owned(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectMemberRole {
    Owner,
    Editor,
    Viewer,
}

impl ProjectMemberRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Editor => "editor",
            Self::Viewer => "viewer",
        }
    }

    pub fn can_update_project(self) -> bool {
        matches!(self, Self::Owner)
    }

    pub fn can_manage_services(self) -> bool {
        matches!(self, Self::Owner | Self::Editor)
    }
}

impl TryFrom<&str> for ProjectMemberRole {
    type Error = InputError;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "owner" => Ok(Self::Owner),
            "editor" => Ok(Self::Editor),
            "viewer" => Ok(Self::Viewer),
            _ => Err(InputError::InvalidMembershipRole),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSummary {
    pub id: ProjectId,
    pub name: String,
    pub owner_id: UserId,
    pub role: ProjectMemberRole,
    pub created_at: String,
    pub updated_at: String,
    pub default_environment: EnvironmentSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentSummary {
    pub id: EnvironmentId,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceKind {
    Image,
    Compose,
}

impl ServiceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Compose => "compose",
        }
    }
}

impl TryFrom<&str> for ServiceKind {
    type Error = InputError;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "image" => Ok(Self::Image),
            "compose" => Ok(Self::Compose),
            _ => Err(InputError::InvalidServiceKind),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ServiceSpec {
    Image {
        image_reference: String,
        internal_port: Option<u32>,
        healthcheck: Option<Vec<String>>,
    },
    Compose {
        yaml: String,
        exposed_service: String,
        internal_port: Option<u32>,
    },
}

impl ServiceSpec {
    pub fn image(
        image_reference: impl Into<String>,
        internal_port: Option<u32>,
        healthcheck: Option<Vec<String>>,
    ) -> Result<Self> {
        let image_reference = image_reference.into();
        let spec = Self::Image {
            image_reference,
            internal_port,
            healthcheck,
        };
        spec.validate()?;
        Ok(spec)
    }

    pub fn compose(
        yaml: impl Into<String>,
        exposed_service: impl AsRef<str>,
        internal_port: Option<u32>,
    ) -> Result<Self> {
        let spec = Self::Compose {
            yaml: yaml.into(),
            exposed_service: exposed_service.as_ref().trim().to_owned(),
            internal_port,
        };
        spec.validate()?;
        Ok(spec)
    }

    pub fn kind(&self) -> ServiceKind {
        match self {
            Self::Image { .. } => ServiceKind::Image,
            Self::Compose { .. } => ServiceKind::Compose,
        }
    }

    pub fn internal_port(&self) -> Option<u32> {
        match self {
            Self::Image { internal_port, .. } | Self::Compose { internal_port, .. } => {
                *internal_port
            }
        }
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Image {
                image_reference,
                internal_port,
                healthcheck,
            } => {
                if !is_digest_image_reference(image_reference) {
                    return Err(InputError::ImageMustUseDigest);
                }
                if internal_port.is_some_and(|port| !(1..=65_535).contains(&port)) {
                    return Err(InputError::InvalidInternalPort);
                }
                if let Some(argv) = healthcheck
                    && (argv.is_empty()
                        || argv
                            .iter()
                            .any(|arg| arg.is_empty() || arg.chars().any(char::is_control)))
                {
                    return Err(InputError::InvalidHealthcheck);
                }
                Ok(())
            }
            Self::Compose {
                yaml,
                exposed_service,
                internal_port,
            } => {
                if yaml.is_empty() || yaml.len() > 1024 * 1024 || yaml.contains('\0') {
                    return Err(InputError::InvalidComposeYaml);
                }
                if !is_dns_label(exposed_service) {
                    return Err(InputError::InvalidComposeExposedService);
                }
                if internal_port.is_some_and(|port| !(1..=65_535).contains(&port)) {
                    return Err(InputError::InvalidInternalPort);
                }
                Ok(())
            }
        }
    }
}

/// Validates an OCI image reference pinned to an exact SHA-256 digest.
pub fn is_digest_image_reference(value: &str) -> bool {
    let Some((name, digest)) = value.split_once("@sha256:") else {
        return false;
    };
    !name.is_empty()
        && !name
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
        && digest.len() == 64
        && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeploymentState {
    Queued,
    Preparing,
    Running,
    Healthy,
    Failed,
    Stopping,
    Stopped,
    Superseded,
}

impl DeploymentState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Preparing => "preparing",
            Self::Running => "running",
            Self::Healthy => "healthy",
            Self::Failed => "failed",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Superseded => "superseded",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Healthy | Self::Failed | Self::Stopped | Self::Superseded
        )
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Queued, Self::Preparing | Self::Failed | Self::Stopped)
                | (
                    Self::Preparing,
                    Self::Running | Self::Failed | Self::Stopping | Self::Stopped
                )
                | (
                    Self::Running,
                    Self::Healthy | Self::Failed | Self::Stopping | Self::Stopped
                )
                | (Self::Healthy, Self::Stopping | Self::Superseded)
                | (Self::Stopping, Self::Stopped | Self::Failed)
        )
    }
}

impl TryFrom<&str> for DeploymentState {
    type Error = InputError;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "queued" => Ok(Self::Queued),
            "preparing" => Ok(Self::Preparing),
            "running" => Ok(Self::Running),
            "healthy" => Ok(Self::Healthy),
            "failed" => Ok(Self::Failed),
            "stopping" => Ok(Self::Stopping),
            "stopped" => Ok(Self::Stopped),
            "superseded" => Ok(Self::Superseded),
            _ => Err(InputError::InvalidDeploymentState),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceConfiguration {
    pub name: String,
    pub spec: ServiceSpec,
    pub source_config: Option<ServiceSourceConfig>,
    pub deployment_destination_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceSourceConfig {
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup_required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builder: Option<ApplicationBuilder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dockerfile_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_directory: Option<String>,
    #[serde(default)]
    pub auto_deploy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApplicationBuilder {
    Static,
    Spa,
    Dockerfile,
    Railpack,
}

impl ServiceSourceConfig {
    pub fn validate(&self) -> Result<()> {
        if !matches!(self.source.as_str(), "template" | "compose" | "application") {
            return Err(InputError::InvalidServiceSourceConfig);
        }
        for value in [
            Some(self.source.as_str()),
            self.template.as_deref(),
            self.provider_id.as_deref(),
            self.repository.as_deref(),
            self.branch.as_deref(),
            self.dockerfile_path.as_deref(),
            self.build_command.as_deref(),
            self.output_directory.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if value.is_empty() || value.len() > 1024 || value.chars().any(char::is_control) {
                return Err(InputError::InvalidServiceSourceConfig);
            }
        }
        if self.source == "application"
            && (self.provider_id.is_none()
                || self.repository.is_none()
                || self.branch.is_none()
                || self.builder.is_none())
        {
            return Err(InputError::InvalidServiceSourceConfig);
        }
        if self.source == "compose"
            && self.provider_id.is_some()
            && (self.repository.is_none() || self.branch.is_none())
        {
            return Err(InputError::InvalidServiceSourceConfig);
        }
        if self.auto_deploy
            && !((self.source == "application")
                || (self.source == "compose" && self.provider_id.is_some()))
        {
            return Err(InputError::InvalidServiceSourceConfig);
        }
        if self.source == "template" && self.template.is_none() {
            return Err(InputError::InvalidServiceSourceConfig);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceVariableInput {
    pub key: String,
    pub value: String,
    pub is_secret: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceInput {
    pub configuration: ServiceConfiguration,
    pub variables: Vec<ServiceVariableInput>,
}

impl ServiceInput {
    pub fn image(
        name: impl AsRef<str>,
        image_reference: impl Into<String>,
        internal_port: Option<u32>,
        healthcheck: Option<Vec<String>>,
        variables: Vec<ServiceVariableInput>,
    ) -> Result<Self> {
        let name = name.as_ref().trim();
        if !is_dns_label(name) {
            return Err(InputError::InvalidServiceName);
        }
        validate_variables(&variables)?;
        Ok(Self {
            configuration: ServiceConfiguration {
                name: name.to_owned(),
                spec: ServiceSpec::image(image_reference, internal_port, healthcheck)?,
                source_config: None,
                deployment_destination_id: None,
            },
            variables,
        })
    }

    pub fn compose(
        name: impl AsRef<str>,
        yaml: impl Into<String>,
        exposed_service: impl AsRef<str>,
        internal_port: Option<u32>,
        variables: Vec<ServiceVariableInput>,
    ) -> Result<Self> {
        let name = name.as_ref().trim();
        if !is_dns_label(name) {
            return Err(InputError::InvalidServiceName);
        }
        validate_variables(&variables)?;
        Ok(Self {
            configuration: ServiceConfiguration {
                name: name.to_owned(),
                spec: ServiceSpec::compose(yaml, exposed_service, internal_port)?,
                source_config: None,
                deployment_destination_id: None,
            },
            variables,
        })
    }
}

fn is_domain_name(value: &str) -> bool {
    if value.len() > 253
        || !value.is_ascii()
        || value == "localhost"
        || value.parse::<std::net::IpAddr>().is_ok()
        || !value.contains('.')
    {
        return false;
    }
    let mut labels = value.split('.');
    if is_public_suffix(value) {
        return false;
    }
    let Some(top_level) = labels.next_back() else {
        return false;
    };
    if top_level.len() < 2 || !top_level.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return false;
    }
    value.split('.').all(is_dns_label)
}

fn is_public_suffix(value: &str) -> bool {
    matches!(
        value,
        "co.uk" | "org.uk" | "ac.uk" | "com.au" | "net.au" | "co.jp"
    )
}

fn is_dns_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
}

fn validate_variables(variables: &[ServiceVariableInput]) -> Result<()> {
    for (index, variable) in variables.iter().enumerate() {
        if variable.key.is_empty()
            || variable.key.len() > 255
            || variable.key.chars().any(char::is_control)
        {
            return Err(InputError::InvalidVariableKey);
        }
        if variable.value.contains('\0') || variable.value.len() > 16 * 1024 {
            return Err(InputError::InvalidVariableValue);
        }
        if variables[..index]
            .iter()
            .any(|prior| prior.key == variable.key)
        {
            return Err(InputError::DuplicateVariableKey);
        }
    }
    Ok(())
}

/// Validates a set of environment variable inputs without tying them to a service configuration.
pub fn validate_variable_inputs(variables: &[ServiceVariableInput]) -> Result<()> {
    validate_variables(variables)
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InputError {
    #[error("invalid identifier")]
    InvalidId,
    #[error("project name must be 1 to 100 characters without control characters")]
    InvalidProjectName,
    #[error("invalid project membership role")]
    InvalidMembershipRole,
    #[error("service name must be a lower-case DNS label")]
    InvalidServiceName,
    #[error("image reference must include a sha256 digest")]
    ImageMustUseDigest,
    #[error("internal port must be between 1 and 65535")]
    InvalidInternalPort,
    #[error("healthcheck must be a non-empty exec-form argument list without control characters")]
    InvalidHealthcheck,
    #[error("invalid service kind")]
    InvalidServiceKind,
    #[error("invalid service source configuration")]
    InvalidServiceSourceConfig,
    #[error("compose YAML must be non-empty, at most 1 MiB, and contain no NUL")]
    InvalidComposeYaml,
    #[error("compose exposed service must be a lower-case DNS label")]
    InvalidComposeExposedService,
    #[error("variable key must be 1 to 255 characters without control characters")]
    InvalidVariableKey,
    #[error("variable keys must be unique")]
    DuplicateVariableKey,
    #[error("variable values must be at most 16 KiB and not contain NUL")]
    InvalidVariableValue,
    #[error("invalid deployment state")]
    InvalidDeploymentState,
    #[error("domain must be a lower-case ASCII fully qualified hostname")]
    InvalidDomainName,
    #[error("invalid domain status")]
    InvalidDomainStatus,
    #[error("invalid DNS record type")]
    InvalidDnsRecordType,
    #[error("invalid DNS record target")]
    InvalidDnsRecordTarget,
    #[error("invalid DNS verification status")]
    InvalidDnsVerificationStatus,
}

pub type Result<T> = std::result::Result<T, InputError>;

#[cfg(test)]
mod tests {
    use super::{
        DeploymentState, DomainName, ProjectId, ProjectInput, ServiceInput, ServiceSpec,
        ServiceVariableInput, is_digest_image_reference,
    };

    #[test]
    fn project_input_trims_valid_name() {
        let input = ProjectInput::new("  App  ").unwrap();
        assert_eq!(input.name, "App");
    }

    #[test]
    fn project_input_rejects_control_character() {
        assert!(ProjectInput::new("bad\nname").is_err());
    }

    #[test]
    fn project_id_rejects_non_uuid_value() {
        assert!(ProjectId::new("project").is_err());
    }

    #[test]
    fn domain_name_accepts_ascii_fqdn_and_rejects_unsafe_values() {
        assert!(DomainName::new("app.example.com").is_ok());
        for value in [
            "*.example.com",
            "https://example.com",
            "example.com/path",
            "example.com:443",
            "127.0.0.1",
            "localhost",
            "com",
            "co.uk",
            "-bad.example.com",
            "bad-.example.com",
        ] {
            assert!(DomainName::new(value).is_err(), "{value}");
        }
    }

    #[test]
    fn deployment_states_allow_only_lifecycle_transitions() {
        assert!(DeploymentState::Queued.can_transition_to(DeploymentState::Preparing));
        assert!(DeploymentState::Queued.can_transition_to(DeploymentState::Stopped));
        assert!(DeploymentState::Running.can_transition_to(DeploymentState::Healthy));
        assert!(DeploymentState::Running.can_transition_to(DeploymentState::Stopping));
        assert!(DeploymentState::Healthy.can_transition_to(DeploymentState::Stopping));
        assert!(!DeploymentState::Queued.can_transition_to(DeploymentState::Running));
        assert!(!DeploymentState::Stopped.can_transition_to(DeploymentState::Running));
    }

    #[test]
    fn image_digest_requires_exact_sha256_grammar() {
        let digest = "a".repeat(64);
        assert!(is_digest_image_reference(&format!("nginx@sha256:{digest}")));
        for value in [
            "nginx:latest",
            "nginx@sha256:deadbeef",
            "nginx@sha512:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "nginx@sha256:gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg",
        ] {
            assert!(!is_digest_image_reference(value), "{value}");
        }
    }

    #[test]
    fn image_service_input_rejects_invalid_configuration() {
        assert!(ServiceInput::image("Bad_Name", "nginx:latest", Some(80), None, vec![]).is_err());
        assert!(ServiceInput::image("web", "nginx:latest", Some(80), None, vec![]).is_err());
        assert!(ServiceSpec::image("nginx@sha256:abc", Some(0), None).is_err());
        assert!(ServiceSpec::image("nginx@sha256:abc", Some(65_536), None).is_err());
        assert!(ServiceSpec::image("nginx@sha256:abc", Some(80), Some(vec![])).is_err());
        assert!(
            ServiceInput::image(
                "web",
                "nginx@sha256:abc",
                None,
                None,
                vec![ServiceVariableInput {
                    key: "TOKEN".to_owned(),
                    value: "x".repeat(16 * 1024 + 1),
                    is_secret: true,
                }],
            )
            .is_err()
        );
        assert!(
            ServiceInput::image(
                "web",
                "nginx@sha256:abc",
                Some(80),
                Some(vec!["ok\n".to_owned()]),
                vec![],
            )
            .is_err()
        );
        assert!(
            ServiceInput::image(
                "web",
                "nginx@sha256:abc",
                Some(80),
                None,
                vec![
                    ServiceVariableInput {
                        key: "PORT".to_owned(),
                        value: "80".to_owned(),
                        is_secret: false
                    },
                    ServiceVariableInput {
                        key: "PORT".to_owned(),
                        value: "81".to_owned(),
                        is_secret: false
                    },
                ],
            )
            .is_err()
        );
    }
}
