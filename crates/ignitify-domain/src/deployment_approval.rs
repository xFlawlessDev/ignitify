use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionApprovalStatus {
    NotRequired,
    Pending,
    Approved,
}

impl ProductionApprovalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "not_required",
            Self::Pending => "pending",
            Self::Approved => "approved",
        }
    }
}

impl TryFrom<&str> for ProductionApprovalStatus {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "not_required" => Ok(Self::NotRequired),
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentApproval {
    pub status: ProductionApprovalStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_by_user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<String>,
}

impl DeploymentApproval {
    pub fn not_required() -> Self {
        Self {
            status: ProductionApprovalStatus::NotRequired,
            requested_at: None,
            approved_by_user_id: None,
            approved_at: None,
        }
    }

    pub fn pending(requested_at: String) -> Self {
        Self {
            status: ProductionApprovalStatus::Pending,
            requested_at: Some(requested_at),
            approved_by_user_id: None,
            approved_at: None,
        }
    }

    pub fn approved(self, user_id: String, approved_at: String) -> Self {
        Self {
            status: ProductionApprovalStatus::Approved,
            requested_at: self.requested_at,
            approved_by_user_id: Some(user_id),
            approved_at: Some(approved_at),
        }
    }

    pub fn is_pending(&self) -> bool {
        self.status == ProductionApprovalStatus::Pending
    }

    pub fn allows_execution(&self) -> bool {
        !self.is_pending()
    }
}

#[cfg(test)]
mod tests {
    use super::{DeploymentApproval, ProductionApprovalStatus};

    #[test]
    fn pending_approval_requires_an_explicit_approval() {
        let pending = DeploymentApproval::pending("2026-08-14T00:00:00Z".to_owned());
        assert!(pending.is_pending());
        assert!(!pending.allows_execution());

        let approved = pending.approved("user-1".to_owned(), "2026-08-14T00:01:00Z".to_owned());
        assert_eq!(approved.status, ProductionApprovalStatus::Approved);
        assert!(approved.allows_execution());
    }
}
