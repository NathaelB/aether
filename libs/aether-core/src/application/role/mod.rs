use crate::{
    AetherService, CoreError,
    organisation::OrganisationId,
    role::{
        Role, RoleId,
        commands::{CreateRoleCommand, UpdateRoleCommand},
        ports::RoleService,
    },
};

impl RoleService for AetherService {
    async fn create_role(&self, command: CreateRoleCommand) -> Result<Role, CoreError> {
        self.role_service.create_role(command).await
    }

    async fn delete_role(&self, role_id: RoleId) -> Result<(), CoreError> {
        self.role_service.delete_role(role_id).await
    }

    async fn get_role(&self, role_id: RoleId) -> Result<Option<Role>, CoreError> {
        self.role_service.get_role(role_id).await
    }

    async fn list_roles_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Role>, CoreError> {
        self.role_service
            .list_roles_by_organisation(organisation_id)
            .await
    }

    async fn update_role(
        &self,
        role_id: RoleId,
        command: UpdateRoleCommand,
    ) -> Result<Role, CoreError> {
        self.role_service.update_role(role_id, command).await
    }
}
