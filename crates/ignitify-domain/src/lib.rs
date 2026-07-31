//! Domain models and validation for Ignitify resources.

use std::{fmt, str::FromStr};

use thiserror::Error;

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
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

uuid_id!(EnvironmentId);
uuid_id!(ProjectId);
uuid_id!(UserId);

fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            matches!(index, 8 | 13 | 18 | 23) && byte == b'-'
                || !matches!(index, 8 | 13 | 18 | 23) && byte.is_ascii_hexdigit()
        })
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

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InputError {
    #[error("invalid identifier")]
    InvalidId,
    #[error("project name must be 1 to 100 characters without control characters")]
    InvalidProjectName,
    #[error("invalid project membership role")]
    InvalidMembershipRole,
}

pub type Result<T> = std::result::Result<T, InputError>;

#[cfg(test)]
mod tests {
    use super::{ProjectId, ProjectInput};

    #[test]
    fn project_input_trims_valid_name() {
        let input = ProjectInput::new("  App  ").unwrap();

        assert_eq!(input.name, "App");
    }

    #[test]
    fn project_input_rejects_control_character() {
        let input = ProjectInput::new("bad\nname");

        assert!(input.is_err());
    }

    #[test]
    fn project_id_rejects_non_uuid_value() {
        let id = ProjectId::new("project");

        assert!(id.is_err());
    }
}
