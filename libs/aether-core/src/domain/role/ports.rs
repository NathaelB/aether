use std::future::Future;

use aether_auth::Identity;
use aether_permission::Permissions;

use crate::{
    CoreError,
    organisation::OrganisationId,
    role::commands::{CreateRoleCommand, UpdateRoleCommand},
    role::{Role, RoleId},
};

/// Service trait for role business logic
pub trait RoleService: Sync + Send {
    /// Creates a new role
    fn create_role(
        &self,
        identity: Identity,
        command: CreateRoleCommand,
    ) -> impl Future<Output = Result<Role, CoreError>> + Send;

    /// Fetches a role by ID
    fn get_role(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        role_id: RoleId,
    ) -> impl Future<Output = Result<Option<Role>, CoreError>> + Send;

    /// Lists roles for an organisation
    fn list_roles_by_organisation(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<Vec<Role>, CoreError>> + Send;

    /// Updates an existing role
    fn update_role(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        role_id: RoleId,
        command: UpdateRoleCommand,
    ) -> impl Future<Output = Result<Role, CoreError>> + Send;

    /// Deletes a role
    fn delete_role(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        role_id: RoleId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;
}

/// Repository trait for Role entity
/// Implement this trait for any data storage mechanism (e.g., database, in-memory)
/// to handle Role entity persistence and retrieval.
#[cfg_attr(any(test, feature = "test-mocks"), mockall::automock)]
pub trait RoleRepository: Send + Sync {
    fn insert(&self, role: Role) -> impl Future<Output = Result<(), CoreError>> + Send;
    fn get_by_id(
        &self,
        role_id: RoleId,
    ) -> impl Future<Output = Result<Option<Role>, CoreError>> + Send;
    fn list_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<Vec<Role>, CoreError>> + Send;
    fn list_by_names(
        &self,
        organisation_id: OrganisationId,
        names: Vec<String>,
    ) -> impl Future<Output = Result<Vec<Role>, CoreError>> + Send;
    fn update(&self, role: Role) -> impl Future<Output = Result<(), CoreError>> + Send;
    fn delete(&self, role_id: RoleId) -> impl Future<Output = Result<(), CoreError>> + Send;
}

#[cfg_attr(any(test, feature = "test-mocks"), mockall::automock)]
pub trait RolePolicy: Send + Sync {
    fn can_view_roles(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn can_manage_roles(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;
}

#[cfg_attr(any(test, feature = "test-mocks"), mockall::automock)]
pub trait PermissionProvider: Send + Sync {
    fn permissions_for_organisation(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<Permissions, CoreError>> + Send;
}

#[cfg(any(test, feature = "test-mocks"))]
impl RoleRepository for std::sync::Arc<MockRoleRepository> {
    fn insert(&self, role: Role) -> impl Future<Output = Result<(), CoreError>> + Send {
        (**self).insert(role)
    }

    fn get_by_id(
        &self,
        role_id: RoleId,
    ) -> impl Future<Output = Result<Option<Role>, CoreError>> + Send {
        (**self).get_by_id(role_id)
    }

    fn list_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<Vec<Role>, CoreError>> + Send {
        (**self).list_by_organisation(organisation_id)
    }

    fn list_by_names(
        &self,
        organisation_id: OrganisationId,
        names: Vec<String>,
    ) -> impl Future<Output = Result<Vec<Role>, CoreError>> + Send {
        (**self).list_by_names(organisation_id, names)
    }

    fn update(&self, role: Role) -> impl Future<Output = Result<(), CoreError>> + Send {
        (**self).update(role)
    }

    fn delete(&self, role_id: RoleId) -> impl Future<Output = Result<(), CoreError>> + Send {
        (**self).delete(role_id)
    }
}
