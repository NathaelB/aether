use aether_auth::Identity;
use tracing::info;

use crate::{
    CoreError,
    application::AetherService,
    infrastructure::{
        organisation::PostgresOrganisationRepository,
        user::PostgresUserRepository,
    },
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
            let user_repository = PostgresUserRepository::from_tx(&tx);
            let organisation_service =
                OrganisationServiceImpl::new(organisation_repository, user_repository);

            organisation_service.create_organisation(command).await
        };

        match result {
            Ok(organisation) => {
                info!("organisation: {:?}", organisation);
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
            let user_repository = PostgresUserRepository::from_tx(&tx);
            let organisation_service =
                OrganisationServiceImpl::new(organisation_repository, user_repository);

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
            let user_repository = PostgresUserRepository::from_tx(&tx);
            let organisation_service =
                OrganisationServiceImpl::new(organisation_repository, user_repository);

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
        let user_repository = PostgresUserRepository::from_pool(self.pool());
        let organisation_service =
            OrganisationServiceImpl::new(organisation_repository, user_repository);

        organisation_service
            .get_organisations(status, limit, offset)
            .await
    }

    async fn get_organisations_by_member(
        &self,
        identity: Identity,
    ) -> Result<Vec<Organisation>, CoreError> {
        let organisation_repository = PostgresOrganisationRepository::from_pool(self.pool());
        let user_repository = PostgresUserRepository::from_pool(self.pool());
        let organisation_service =
            OrganisationServiceImpl::new(organisation_repository, user_repository);

        organisation_service
            .get_organisations_by_member(identity)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::organisation::value_objects::{OrganisationName, Plan};
    use crate::domain::user::UserId;
    use aether_auth::Identity;
    use sqlx::postgres::PgPoolOptions;
    use std::time::Duration;
    use uuid::Uuid;

    fn service() -> AetherService {
        let pool = PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(50))
            .connect_lazy("postgres://user:pass@127.0.0.1:1/db")
            .expect("valid database url");
        AetherService::new(pool)
    }

    #[tokio::test]
    async fn create_organisation_maps_pool_error() {
        let command = CreateOrganisationCommand::new(
            OrganisationName::new("Acme Corp").unwrap(),
            "user-sub-1".to_string(),
            Plan::Free,
        );

        let result = service().create_organisation(command).await;
        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn get_organisations_maps_pool_error() {
        let result = service().get_organisations(None, 10, 0).await;
        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn get_organisations_by_member_rejects_invalid_identity() {
        let identity = Identity::User(aether_auth::User {
            id: "not-a-uuid".to_string(),
            username: "user".to_string(),
            email: None,
            name: None,
            roles: vec![],
        });

        let result = service().get_organisations_by_member(identity).await;
        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn delete_organisation_maps_pool_error() {
        let result = service()
            .delete_organisation(OrganisationId(Uuid::new_v4()))
            .await;
        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn update_organisation_maps_pool_error() {
        let command = UpdateOrganisationCommand::new();
        let result = service()
            .update_organisation(OrganisationId(Uuid::new_v4()), command)
            .await;
        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }
}
