use aether_auth::Identity;
use aether_macros::transactional;

use crate::{
    CoreError,
    application::AetherService,
    organisation::service::OrganisationServiceImpl,
    organisation::{
        Organisation, OrganisationId,
        commands::{CreateOrganisationCommand, UpdateOrganisationCommand},
        ports::OrganisationService,
        value_objects::OrganisationStatus,
    },
};

impl OrganisationService for AetherService {
    #[transactional(organisation, user)]
    async fn create_organisation(
        &self,
        command: CreateOrganisationCommand,
    ) -> Result<Organisation, CoreError> {
        OrganisationServiceImpl::new(organisation_repository, user_repository)
            .create_organisation(command)
            .await
    }

    #[transactional(organisation, user)]
    async fn delete_organisation(&self, id: OrganisationId) -> Result<(), CoreError> {
        OrganisationServiceImpl::new(organisation_repository, user_repository)
            .delete_organisation(id)
            .await
    }

    #[transactional(organisation, user)]
    async fn update_organisation(
        &self,
        id: OrganisationId,
        command: UpdateOrganisationCommand,
    ) -> Result<Organisation, CoreError> {
        OrganisationServiceImpl::new(organisation_repository, user_repository)
            .update_organisation(id, command)
            .await
    }

    #[transactional(organisation, user)]
    async fn get_organisations(
        &self,
        status: Option<OrganisationStatus>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Organisation>, CoreError> {
        OrganisationServiceImpl::new(organisation_repository, user_repository)
            .get_organisations(status, limit, offset)
            .await
    }

    #[transactional(organisation, user)]
    async fn get_organisations_by_member(
        &self,
        identity: Identity,
    ) -> Result<Vec<Organisation>, CoreError> {
        OrganisationServiceImpl::new(organisation_repository, user_repository)
            .get_organisations_by_member(identity)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::organisation::value_objects::{OrganisationName, Plan};
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
