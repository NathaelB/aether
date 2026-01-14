use aether_auth::Identity;

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
    async fn create_role(
        &self,
        identity: Identity,
        command: CreateRoleCommand,
    ) -> Result<Role, CoreError> {
        self.role_service.create_role(identity, command).await
    }

    async fn delete_role(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        role_id: RoleId,
    ) -> Result<(), CoreError> {
        self.role_service
            .delete_role(identity, organisation_id, role_id)
            .await
    }

    async fn get_role(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        role_id: RoleId,
    ) -> Result<Option<Role>, CoreError> {
        self.role_service
            .get_role(identity, organisation_id, role_id)
            .await
    }

    async fn list_roles_by_organisation(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Role>, CoreError> {
        self.role_service
            .list_roles_by_organisation(identity, organisation_id)
            .await
    }

    async fn update_role(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        role_id: RoleId,
        command: UpdateRoleCommand,
    ) -> Result<Role, CoreError> {
        self.role_service
            .update_role(identity, organisation_id, role_id, command)
            .await
    }
}
