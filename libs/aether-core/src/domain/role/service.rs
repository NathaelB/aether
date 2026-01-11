use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use crate::{
    CoreError,
    organisation::OrganisationId,
    role::{
        Role, RoleId,
        commands::{CreateRoleCommand, UpdateRoleCommand},
        ports::{RoleRepository, RoleService},
    },
};

#[derive(Clone)]
pub struct RoleServiceImpl<R>
where
    R: RoleRepository,
{
    role_repository: Arc<R>,
}

impl<R> RoleServiceImpl<R>
where
    R: RoleRepository,
{
    pub fn new(role_repository: Arc<R>) -> Self {
        Self { role_repository }
    }
}

impl<R> RoleService for RoleServiceImpl<R>
where
    R: RoleRepository,
{
    async fn create_role(&self, command: CreateRoleCommand) -> Result<Role, CoreError> {
        let role = Role {
            id: RoleId(Uuid::new_v4()),
            name: command.name,
            permissions: command.permissions,
            organisation_id: command.organisation_id,
            color: command.color,
            created_at: Utc::now(),
        };

        self.role_repository.insert(role.clone()).await?;
        Ok(role)
    }

    async fn delete_role(&self, role_id: RoleId) -> Result<(), CoreError> {
        self.role_repository.delete(role_id).await
    }

    async fn get_role(&self, role_id: RoleId) -> Result<Option<Role>, CoreError> {
        self.role_repository.get_by_id(role_id).await
    }

    async fn list_roles_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Role>, CoreError> {
        self.role_repository
            .list_by_organisation(organisation_id)
            .await
    }

    async fn update_role(
        &self,
        role_id: RoleId,
        command: UpdateRoleCommand,
    ) -> Result<Role, CoreError> {
        if command.is_empty() {
            return Err(CoreError::InternalError(
                "Update command cannot be empty".to_string(),
            ));
        }

        let mut role = self
            .role_repository
            .get_by_id(role_id)
            .await?
            .ok_or(CoreError::InternalError("Role not found".to_string()))?;

        if let Some(name) = command.name {
            role.name = name;
        }
        if let Some(permissions) = command.permissions {
            role.permissions = permissions;
        }
        if let Some(organisation_id) = command.organisation_id {
            role.organisation_id = Some(organisation_id);
        }
        if let Some(color) = command.color {
            role.color = Some(color);
        }

        self.role_repository.update(role.clone()).await?;
        Ok(role)
    }
}
