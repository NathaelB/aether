use std::fmt;
use std::str::FromStr;

use serde::Serialize;
use utoipa::ToSchema;

use crate::CoreError;

/// Organisation name value object
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct OrganisationName(String);

impl OrganisationName {
    /// Creates a new OrganisationName
    ///
    /// # Validation Rules
    /// - Must be between 3 and 100 characters
    /// - Cannot be empty or only whitespace
    pub fn new(name: impl Into<String>) -> Result<Self, CoreError> {
        let name = name.into().trim().to_string();

        if name.is_empty() {
            return Err(CoreError::InvalidOrganisationName {
                reason: "Name cannot be empty".to_string(),
            });
        }

        if name.len() < 3 {
            return Err(CoreError::InvalidOrganisationName {
                reason: format!(
                    "Name must be at least 3 characters long, got {}",
                    name.len()
                ),
            });
        }

        if name.len() > 100 {
            return Err(CoreError::InvalidOrganisationName {
                reason: format!("Name cannot exceed 100 characters, got {}", name.len()),
            });
        }

        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OrganisationName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Organisation slug value object (used in URLs)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct OrganisationSlug(String);

impl OrganisationSlug {
    /// Creates a new OrganisationSlug
    ///
    /// # Validation Rules
    /// - Must be between 3 and 50 characters
    /// - Only lowercase alphanumeric characters and hyphens
    /// - Cannot start or end with a hyphen
    pub fn new(slug: impl Into<String>) -> Result<Self, CoreError> {
        let slug = slug.into().to_lowercase().trim().to_string();

        if slug.is_empty() {
            return Err(CoreError::InvalidOrganisationSlug {
                reason: "Slug cannot be empty".to_string(),
            });
        }

        if slug.len() < 3 {
            return Err(CoreError::InvalidOrganisationSlug {
                reason: format!(
                    "Slug must be at least 3 characters long, got {}",
                    slug.len()
                ),
            });
        }

        if slug.len() > 50 {
            return Err(CoreError::InvalidOrganisationSlug {
                reason: format!("Slug cannot exceed 50 characters, got {}", slug.len()),
            });
        }

        if slug.starts_with('-') || slug.ends_with('-') {
            return Err(CoreError::InvalidOrganisationSlug {
                reason: "Slug cannot start or end with a hyphen".to_string(),
            });
        }

        if !slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(CoreError::InvalidOrganisationSlug {
                reason: "Slug can only contain lowercase alphanumeric characters and hyphens"
                    .to_string(),
            });
        }

        Ok(Self(slug))
    }

    /// Generates a slug from a name
    pub fn from_name(name: &OrganisationName) -> Result<Self, CoreError> {
        let slug = name
            .as_str()
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>()
            // Replace multiple consecutive hyphens with a single one
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");

        Self::new(slug)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OrganisationSlug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Organisation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub enum OrganisationStatus {
    /// Organisation is active and can be used
    Active,
    /// Organisation is suspended (payment issues, violations, etc.)
    Suspended,
    /// Organisation is marked for deletion
    Deleted,
}

impl OrganisationStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }

    pub fn is_suspended(&self) -> bool {
        matches!(self, Self::Suspended)
    }

    pub fn is_deleted(&self) -> bool {
        matches!(self, Self::Deleted)
    }
}

impl fmt::Display for OrganisationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Suspended => write!(f, "suspended"),
            Self::Deleted => write!(f, "deleted"),
        }
    }
}

impl FromStr for OrganisationStatus {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "suspended" => Ok(Self::Suspended),
            "deleted" => Ok(Self::Deleted),
            _ => Err(CoreError::InvalidOrganisationStatus {
                value: s.to_string(),
            }),
        }
    }
}

/// Subscription plan for an organisation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub enum Plan {
    /// Free tier with basic features
    Free,
    /// Starter plan for small teams
    Starter,
    /// Business plan for growing companies
    Business,
    /// Enterprise plan with custom features
    Enterprise,
}

impl Plan {
    pub fn max_instances(&self) -> usize {
        match self {
            Self::Free => 1,
            Self::Starter => 5,
            Self::Business => 20,
            Self::Enterprise => 100,
        }
    }

    pub fn max_users(&self) -> usize {
        match self {
            Self::Free => 2,
            Self::Starter => 10,
            Self::Business => 50,
            Self::Enterprise => 100,
        }
    }

    pub fn max_storage_gb(&self) -> usize {
        match self {
            Self::Free => 1,
            Self::Starter => 10,
            Self::Business => 50,
            Self::Enterprise => 100,
        }
    }
}

impl fmt::Display for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Free => write!(f, "free"),
            Self::Starter => write!(f, "starter"),
            Self::Business => write!(f, "business"),
            Self::Enterprise => write!(f, "enterprise"),
        }
    }
}

impl FromStr for Plan {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "free" => Ok(Self::Free),
            "starter" => Ok(Self::Starter),
            "business" => Ok(Self::Business),
            "enterprise" => Ok(Self::Enterprise),
            _ => Err(CoreError::InvalidPlan {
                value: s.to_string(),
            }),
        }
    }
}

/// Resource limits for an organisation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct OrganisationLimits {
    pub max_instances: usize,
    pub max_users: usize,
    pub max_storage_gb: usize,
}

impl OrganisationLimits {
    pub fn from_plan(plan: &Plan) -> Self {
        Self {
            max_instances: plan.max_instances(),
            max_users: plan.max_users(),
            max_storage_gb: plan.max_storage_gb(),
        }
    }

    pub fn custom(max_instances: usize, max_users: usize, max_storage_gb: usize) -> Self {
        Self {
            max_instances,
            max_users,
            max_storage_gb,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_organisation_name_validation() {
        // Valid names
        assert!(OrganisationName::new("Acme Corp").is_ok());
        assert!(OrganisationName::new("My Organisation").is_ok());

        // Invalid names
        assert!(OrganisationName::new("").is_err());
        assert!(OrganisationName::new("  ").is_err());
        assert!(OrganisationName::new("AB").is_err()); // Too short
        assert!(OrganisationName::new("a".repeat(101)).is_err()); // Too long
    }

    #[test]
    fn test_organisation_slug_validation() {
        // Valid slugs
        assert!(OrganisationSlug::new("acme-corp").is_ok());
        assert!(OrganisationSlug::new("my-org-123").is_ok());

        // Invalid slugs
        assert!(OrganisationSlug::new("").is_err());
        assert!(OrganisationSlug::new("ab").is_err()); // Too short
        assert!(OrganisationSlug::new("-acme").is_err()); // Starts with hyphen
        assert!(OrganisationSlug::new("acme-").is_err()); // Ends with hyphen
        assert!(OrganisationSlug::new("acme corp").is_err()); // Contains space
        assert!(OrganisationSlug::new("acme_corp").is_err()); // Contains underscore
    }

    #[test]
    fn test_slug_from_name() {
        let name = OrganisationName::new("Acme Corp!").unwrap();
        let slug = OrganisationSlug::from_name(&name).unwrap();
        assert_eq!(slug.as_str(), "acme-corp");

        let name = OrganisationName::new("My Awesome Organisation 123").unwrap();
        let slug = OrganisationSlug::from_name(&name).unwrap();
        assert_eq!(slug.as_str(), "my-awesome-organisation-123");
    }

    #[test]
    fn test_plan_limits() {
        assert_eq!(Plan::Free.max_instances(), 1);
        assert_eq!(Plan::Starter.max_instances(), 5);
        assert_eq!(Plan::Business.max_instances(), 20);

        assert_eq!(Plan::Free.max_users(), 2);
        assert_eq!(Plan::Enterprise.max_users(), 100);
    }
}
