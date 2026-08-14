use serde::{Deserialize, Serialize};

use crate::{ServiceSourceConfig, ServiceSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SupplyChainCheckStatus {
    Pass,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SupplyChainEnforcement {
    Warning,
    RequireProvenance,
}

impl SupplyChainEnforcement {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::RequireProvenance => "require-provenance",
        }
    }
}

impl TryFrom<&str> for SupplyChainEnforcement {
    type Error = crate::InputError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "warning" => Ok(Self::Warning),
            "require-provenance" => Ok(Self::RequireProvenance),
            _ => Err(crate::InputError::InvalidSupplyChainEnforcement),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupplyChainPolicy {
    pub enforcement: SupplyChainEnforcement,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupplyChainCheck {
    pub status: SupplyChainCheckStatus,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupplyChainReport {
    pub enforcement: SupplyChainEnforcement,
    pub status: SupplyChainCheckStatus,
    pub provenance: SupplyChainCheck,
    pub sbom: SupplyChainCheck,
    pub vulnerabilities: SupplyChainCheck,
    pub evaluated_at: String,
}

impl SupplyChainReport {
    pub fn blocks_execution(&self) -> bool {
        self.enforcement == SupplyChainEnforcement::RequireProvenance
            && self.provenance.status == SupplyChainCheckStatus::Warning
    }
}

/// Produces an informational policy snapshot from the identities Ignitify has
/// actually resolved. Missing external evidence is a warning, never a pass.
pub fn evaluate_supply_chain_report(
    spec: &ServiceSpec,
    source_config: Option<&ServiceSourceConfig>,
    source_revision: Option<&str>,
    local_image_id: Option<&str>,
    enforcement: SupplyChainEnforcement,
    evaluated_at: String,
) -> SupplyChainReport {
    let has_resolved_build_identity = source_revision.is_some() && local_image_id.is_some();
    let requires_source_build = source_config.is_some_and(|config| {
        config.source == "application"
            || (config.source == "compose" && config.provider_id.is_some())
    });
    let provenance = match (has_resolved_build_identity, requires_source_build) {
        (true, _) => SupplyChainCheck {
            status: SupplyChainCheckStatus::Pass,
            summary: "A source revision and built image digest are recorded.".to_owned(),
            remediation: None,
        },
        (false, false) if matches!(spec, ServiceSpec::Image { .. }) => SupplyChainCheck {
            status: SupplyChainCheckStatus::Pass,
            summary: "The runtime image is pinned to an immutable digest.".to_owned(),
            remediation: None,
        },
        _ => SupplyChainCheck {
            status: SupplyChainCheckStatus::Warning,
            summary: "No resolved source revision and image digest are both recorded yet."
                .to_owned(),
            remediation: Some(
                "Rebuild from a pinned source revision and retain the resolved image digest."
                    .to_owned(),
            ),
        },
    };

    let sbom = SupplyChainCheck {
        status: SupplyChainCheckStatus::Warning,
        summary: "No verified application-image SBOM is attached to this deployment.".to_owned(),
        remediation: Some(
            "Attach and verify a CycloneDX or SPDX SBOM for the resolved image digest.".to_owned(),
        ),
    };
    let vulnerabilities = SupplyChainCheck {
        status: SupplyChainCheckStatus::Warning,
        summary: "No vulnerability scan result is attached to this deployment.".to_owned(),
        remediation: Some(
            "Record a vulnerability scan for the resolved image digest before enforcing policy."
                .to_owned(),
        ),
    };

    SupplyChainReport {
        enforcement,
        status: SupplyChainCheckStatus::Warning,
        provenance,
        sbom,
        vulnerabilities,
        evaluated_at,
    }
}

#[cfg(test)]
mod tests {
    use super::{SupplyChainCheckStatus, SupplyChainEnforcement, evaluate_supply_chain_report};
    use crate::ServiceSpec;

    const DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn reports_pinned_runtime_image_provenance_without_claiming_external_evidence() {
        let spec = ServiceSpec::image(format!("registry.example/app@{DIGEST}"), Some(8080), None)
            .expect("digest image is valid");

        let report = evaluate_supply_chain_report(
            &spec,
            None,
            None,
            None,
            SupplyChainEnforcement::Warning,
            "2026-08-14T00:00:00Z".into(),
        );

        assert_eq!(report.enforcement, super::SupplyChainEnforcement::Warning);
        assert_eq!(report.status, SupplyChainCheckStatus::Warning);
        assert_eq!(report.provenance.status, SupplyChainCheckStatus::Pass);
        assert_eq!(report.sbom.status, SupplyChainCheckStatus::Warning);
        assert_eq!(
            report.vulnerabilities.status,
            SupplyChainCheckStatus::Warning
        );
    }

    #[test]
    fn reports_source_build_provenance_only_when_both_identities_are_available() {
        let spec =
            ServiceSpec::compose("services: {}", "app", Some(8080)).expect("compose spec is valid");

        let incomplete = evaluate_supply_chain_report(
            &spec,
            None,
            Some(&"a".repeat(40)),
            None,
            SupplyChainEnforcement::Warning,
            "2026-08-14T00:00:00Z".into(),
        );
        let resolved = evaluate_supply_chain_report(
            &spec,
            None,
            Some(&"a".repeat(40)),
            Some(DIGEST),
            SupplyChainEnforcement::Warning,
            "2026-08-14T00:01:00Z".into(),
        );

        assert_eq!(
            incomplete.provenance.status,
            SupplyChainCheckStatus::Warning
        );
        assert_eq!(resolved.provenance.status, SupplyChainCheckStatus::Pass);
    }

    #[test]
    fn leaves_application_build_provenance_as_warning_until_the_build_is_resolved() {
        let spec = ServiceSpec::image(format!("registry.example/app@{DIGEST}"), Some(8080), None)
            .expect("digest image is valid");
        let source_config = crate::ServiceSourceConfig {
            source: "application".to_owned(),
            template: None,
            provider_id: Some("provider-id".to_owned()),
            repository: Some("acme/app".to_owned()),
            branch: Some("main".to_owned()),
            builder: Some(crate::ApplicationBuilder::Dockerfile),
            dockerfile_path: None,
            build_command: None,
            output_directory: None,
            auto_deploy: false,
            setup_required: Some(false),
        };

        let report = evaluate_supply_chain_report(
            &spec,
            Some(&source_config),
            None,
            None,
            SupplyChainEnforcement::Warning,
            "2026-08-14T00:00:00Z".into(),
        );

        assert_eq!(report.provenance.status, SupplyChainCheckStatus::Warning);
    }

    #[test]
    fn require_provenance_blocks_only_an_unresolved_provenance_check() {
        let spec =
            ServiceSpec::compose("services: {}", "app", Some(8080)).expect("compose spec is valid");
        let blocked = evaluate_supply_chain_report(
            &spec,
            None,
            None,
            None,
            SupplyChainEnforcement::RequireProvenance,
            "2026-08-14T00:00:00Z".into(),
        );
        let permitted = evaluate_supply_chain_report(
            &spec,
            None,
            Some(&"a".repeat(40)),
            Some(DIGEST),
            SupplyChainEnforcement::RequireProvenance,
            "2026-08-14T00:00:00Z".into(),
        );

        assert!(blocked.blocks_execution());
        assert!(!permitted.blocks_execution());
    }
}
