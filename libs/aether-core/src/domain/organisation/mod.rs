use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::{CoreError, user::UserId};

pub mod commands;
pub mod ports;
pub mod service;
pub mod value_objects;

use value_objects::{
    OrganisationLimits, OrganisationName, OrganisationSlug, OrganisationStatus, Plan,
};

/// Organisation ID value object
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, ToSchema)]
pub struct OrganisationId(pub Uuid);

impl OrganisationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for OrganisationId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for OrganisationId {
    fn from(uuid: Uuid) -> Self {
        OrganisationId(uuid)
    }
}

impl std::fmt::Display for OrganisationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Organisation aggregate root
#[derive(Debug, Clone, Serialize, PartialEq, ToSchema)]
pub struct Organisation {
    pub id: OrganisationId,
    pub name: OrganisationName,
    pub slug: OrganisationSlug,
    pub owner_id: UserId,
    pub status: OrganisationStatus,
    pub plan: Plan,
    pub limits: OrganisationLimits,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Organisation {
    /// Creates a new organisation
    pub fn new(
        name: OrganisationName,
        slug: OrganisationSlug,
        owner_id: UserId,
        plan: Plan,
    ) -> Self {
        let now = Utc::now();
        let limits = OrganisationLimits::from_plan(&plan);

        Self {
            id: OrganisationId::new(),
            name,
            slug,
            owner_id,
            status: OrganisationStatus::Active,
            plan,
            limits,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    /// Creates an organisation with custom limits
    pub fn new_with_custom_limits(
        name: OrganisationName,
        slug: OrganisationSlug,
        owner_id: UserId,
        plan: Plan,
        limits: OrganisationLimits,
    ) -> Self {
        let now = Utc::now();

        Self {
            id: OrganisationId::new(),
            name,
            slug,
            owner_id,
            status: OrganisationStatus::Active,
            plan,
            limits,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    /// Checks if the organisation is active
    pub fn is_active(&self) -> bool {
        self.status.is_active() && self.deleted_at.is_none()
    }

    /// Checks if the organisation is suspended
    pub fn is_suspended(&self) -> bool {
        self.status.is_suspended()
    }

    /// Checks if the organisation is deleted
    pub fn is_deleted(&self) -> bool {
        self.status.is_deleted() || self.deleted_at.is_some()
    }

    /// Suspends the organisation
    pub fn suspend(&mut self) -> Result<(), CoreError> {
        if self.is_deleted() {
            return Err(CoreError::InternalError(
                "Cannot suspend a deleted organisation".to_string(),
            ));
        }

        self.status = OrganisationStatus::Suspended;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Reactivates a suspended organisation
    pub fn activate(&mut self) -> Result<(), CoreError> {
        if self.is_deleted() {
            return Err(CoreError::InternalError(
                "Cannot activate a deleted organisation".to_string(),
            ));
        }

        self.status = OrganisationStatus::Active;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Soft deletes the organisation
    pub fn delete(&mut self) -> Result<(), CoreError> {
        if self.is_deleted() {
            return Err(CoreError::InternalError(
                "Organisation is already deleted".to_string(),
            ));
        }

        self.status = OrganisationStatus::Deleted;
        self.deleted_at = Some(Utc::now());
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Updates the organisation name and slug
    pub fn update_name(&mut self, name: OrganisationName, slug: OrganisationSlug) {
        self.name = name;
        self.slug = slug;
        self.updated_at = Utc::now();
    }

    /// Upgrades the organisation plan
    pub fn upgrade_plan(&mut self, new_plan: Plan) -> Result<(), CoreError> {
        if !self.is_active() {
            return Err(CoreError::OrganisationSuspended {
                reason: "Cannot upgrade plan for a non-active organisation".to_string(),
            });
        }

        self.plan = new_plan;
        self.limits = OrganisationLimits::from_plan(&new_plan);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Sets custom limits for the organisation
    pub fn set_custom_limits(&mut self, limits: OrganisationLimits) {
        self.limits = limits;
        self.updated_at = Utc::now();
    }

    /// Checks if a resource limit is reached
    pub fn check_instance_limit(&self, current_count: usize) -> Result<(), CoreError> {
        if current_count >= self.limits.max_instances {
            return Err(CoreError::OrganisationLimitReached {
                limit_type: "instances".to_string(),
                max: self.limits.max_instances,
                current: current_count,
            });
        }
        Ok(())
    }

    pub fn check_user_limit(&self, current_count: usize) -> Result<(), CoreError> {
        if current_count >= self.limits.max_users {
            return Err(CoreError::OrganisationLimitReached {
                limit_type: "users".to_string(),
                max: self.limits.max_users,
                current: current_count,
            });
        }
        // None means unlimited, so always Ok
        Ok(())
    }

    pub fn check_storage_limit(&self, current_gb: usize) -> Result<(), CoreError> {
        if current_gb >= self.limits.max_storage_gb {
            return Err(CoreError::OrganisationLimitReached {
                limit_type: "storage".to_string(),
                max: self.limits.max_storage_gb,
                current: current_gb,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_organisation() {
        let name = OrganisationName::new("Acme Corp").unwrap();
        let slug = OrganisationSlug::new("acme-corp").unwrap();
        let owner_id = UserId(Uuid::new_v4());

        let org = Organisation::new(name.clone(), slug.clone(), owner_id, Plan::Starter);

        assert_eq!(org.name, name);
        assert_eq!(org.slug, slug);
        assert!(org.is_active());
        assert_eq!(org.plan, Plan::Starter);
        assert_eq!(org.limits.max_instances, 5);
    }

    #[test]
    fn test_organisation_suspension() {
        let name = OrganisationName::new("Test Org").unwrap();
        let slug = OrganisationSlug::new("test-org").unwrap();
        let owner_id = UserId(Uuid::new_v4());

        let mut org = Organisation::new(name, slug, owner_id, Plan::Free);

        assert!(org.is_active());
        assert!(org.suspend().is_ok());
        assert!(org.is_suspended());
        assert!(!org.is_active());
    }

    #[test]
    fn test_organisation_deletion() {
        let name = OrganisationName::new("Test Org").unwrap();
        let slug = OrganisationSlug::new("test-org").unwrap();
        let owner_id = UserId(Uuid::new_v4());

        let mut org = Organisation::new(name, slug, owner_id, Plan::Free);

        assert!(org.delete().is_ok());
        assert!(org.is_deleted());
        assert!(org.deleted_at.is_some());
    }

    #[test]
    fn test_upgrade_plan() {
        let name = OrganisationName::new("Test Org").unwrap();
        let slug = OrganisationSlug::new("test-org").unwrap();
        let owner_id = UserId(Uuid::new_v4());

        let mut org = Organisation::new(name, slug, owner_id, Plan::Free);

        assert_eq!(org.limits.max_instances, 1);
        assert!(org.upgrade_plan(Plan::Business).is_ok());
        assert_eq!(org.plan, Plan::Business);
        assert_eq!(org.limits.max_instances, 20);
    }

    #[test]
    fn test_limit_checks() {
        let name = OrganisationName::new("Test Org").unwrap();
        let slug = OrganisationSlug::new("test-org").unwrap();
        let owner_id = UserId(Uuid::new_v4());

        let org = Organisation::new(name, slug, owner_id, Plan::Free);

        // Free plan allows 1 instance
        assert!(org.check_instance_limit(0).is_ok());
        assert!(org.check_instance_limit(1).is_err());

        // Free plan allows 5 users
        assert!(org.check_user_limit(1).is_ok());
        assert!(org.check_user_limit(2).is_err());
    }
}
