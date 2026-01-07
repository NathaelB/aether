use crate::{
    CoreError,
    application::AetherService,
    organisation::{
        Organisation, OrganisationId,
        commands::{CreateOrganisationCommand, UpdateOrganisationCommand},
        ports::OrganisationService,
        value_objects::OrganisationStatus,
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

    async fn get_organisations(
        &self,
        status: Option<OrganisationStatus>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Organisation>, CoreError> {
        self.organisation_service
            .get_organisations(status, limit, offset)
            .await
    }
}
