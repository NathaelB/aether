use crate::{
    CoreError,
    application::AetherService,
    organisation::{
        Organisation, OrganisationId,
        commands::{CreateOrganisationCommand, UpdateOrganisationCommand},
        ports::OrganisationService,
    },
};

impl OrganisationService for AetherService {
    async fn create_organisation(
        &self,
        command: CreateOrganisationCommand,
    ) -> Result<Organisation, CoreError> {
        self.organisation_service.create_organisation(command).await
    }

    async fn delete_organisation(&self, id: OrganisationId) -> Result<(), CoreError> {
        self.organisation_service.delete_organisation(id).await
    }

    async fn update_organisation(
        &self,
        id: OrganisationId,
        command: UpdateOrganisationCommand,
    ) -> Result<Organisation, CoreError> {
        self.organisation_service
            .update_organisation(id, command)
            .await
    }
}
