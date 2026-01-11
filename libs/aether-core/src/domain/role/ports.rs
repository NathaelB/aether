use std::future::Future;

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
        command: CreateRoleCommand,
    ) -> impl Future<Output = Result<Role, CoreError>> + Send;

    /// Fetches a role by ID
    fn get_role(
        &self,
        role_id: RoleId,
    ) -> impl Future<Output = Result<Option<Role>, CoreError>> + Send;

    /// Lists roles for an organisation
    fn list_roles_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<Vec<Role>, CoreError>> + Send;

    /// Updates an existing role
    fn update_role(
        &self,
        role_id: RoleId,
        command: UpdateRoleCommand,
    ) -> impl Future<Output = Result<Role, CoreError>> + Send;

    /// Deletes a role
    fn delete_role(&self, role_id: RoleId) -> impl Future<Output = Result<(), CoreError>> + Send;
}

/// Repository trait for Role entity
/// Implement this trait for any data storage mechanism (e.g., database, in-memory)
/// to handle Role entity persistence and retrieval.
#[cfg_attr(test, mockall::automock)]
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
    fn update(&self, role: Role) -> impl Future<Output = Result<(), CoreError>> + Send;
    fn delete(&self, role_id: RoleId) -> impl Future<Output = Result<(), CoreError>> + Send;
}
