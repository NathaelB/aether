use aether_auth::Identity;

use crate::{
    CoreError,
    application::AetherService,
    infrastructure::organisation::PostgresOrganisationRepository,
    organisation::service::OrganisationServiceImpl,
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
        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = tokio::sync::Mutex::new(Some(tx));

        let result = {
            let organisation_repository = PostgresOrganisationRepository::from_tx(&tx);
            let organisation_service = OrganisationServiceImpl::new(organisation_repository);

            organisation_service.create_organisation(command).await
        };

        match result {
            Ok(organisation) => {
                super::take_transaction(&tx)
                    .await?
                    .commit()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Ok(organisation)
            }
            Err(err) => {
                super::take_transaction(&tx)
                    .await?
                    .rollback()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Err(err)
            }
        }
    }

    async fn delete_organisation(&self, id: OrganisationId) -> Result<(), CoreError> {
        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = tokio::sync::Mutex::new(Some(tx));

        let result = {
            let organisation_repository = PostgresOrganisationRepository::from_tx(&tx);
            let organisation_service = OrganisationServiceImpl::new(organisation_repository);

            organisation_service.delete_organisation(id).await
        };

        match result {
            Ok(()) => {
                super::take_transaction(&tx)
                    .await?
                    .commit()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Ok(())
            }
            Err(err) => {
                super::take_transaction(&tx)
                    .await?
                    .rollback()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Err(err)
            }
        }
    }

    async fn update_organisation(
        &self,
        id: OrganisationId,
        command: UpdateOrganisationCommand,
    ) -> Result<Organisation, CoreError> {
        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = tokio::sync::Mutex::new(Some(tx));

        let result = {
            let organisation_repository = PostgresOrganisationRepository::from_tx(&tx);
            let organisation_service = OrganisationServiceImpl::new(organisation_repository);

            organisation_service.update_organisation(id, command).await
        };

        match result {
            Ok(organisation) => {
                super::take_transaction(&tx)
                    .await?
                    .commit()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Ok(organisation)
            }
            Err(err) => {
                super::take_transaction(&tx)
                    .await?
                    .rollback()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Err(err)
            }
        }
    }

    async fn get_organisations(
        &self,
        status: Option<OrganisationStatus>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Organisation>, CoreError> {
        let organisation_repository = PostgresOrganisationRepository::from_pool(self.pool());
        let organisation_service = OrganisationServiceImpl::new(organisation_repository);

        organisation_service
            .get_organisations(status, limit, offset)
            .await
    }

    async fn get_organisations_by_member(
        &self,
        identity: Identity,
    ) -> Result<Vec<Organisation>, CoreError> {
        let organisation_repository = PostgresOrganisationRepository::from_pool(self.pool());
        let organisation_service = OrganisationServiceImpl::new(organisation_repository);

        organisation_service
            .get_organisations_by_member(identity)
            .await
    }
}
