use aether_auth::Identity;

use crate::{
    AetherService, CoreError,
    infrastructure::role::{PostgresRoleRepository, RolePermissionProvider},
    organisation::OrganisationId,
    policy::AetherPolicy,
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
        #[cfg(feature = "test-mocks")]
        if let Some(role_repo) = crate::test_mocks::role_repository() {
            let policy_repo = role_repo.clone();
            let role_policy = AetherPolicy::new(RolePermissionProvider::new(policy_repo));
            let role_service = crate::role::service::RoleServiceImpl::new(role_repo, role_policy);

            return role_service.create_role(identity, command).await;
        }

        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = tokio::sync::Mutex::new(Some(tx));

        let result = {
            let role_repo = PostgresRoleRepository::from_tx(&tx);
            let policy_repo = PostgresRoleRepository::from_pool(self.pool());
            let role_policy = AetherPolicy::new(RolePermissionProvider::new(policy_repo));
            let role_service = crate::role::service::RoleServiceImpl::new(role_repo, role_policy);

            role_service.create_role(identity, command).await
        };

        match result {
            Ok(role) => {
                super::take_transaction(&tx)
                    .await?
                    .commit()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Ok(role)
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

    async fn delete_role(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        role_id: RoleId,
    ) -> Result<(), CoreError> {
        #[cfg(feature = "test-mocks")]
        if let Some(role_repo) = crate::test_mocks::role_repository() {
            let policy_repo = role_repo.clone();
            let role_policy = AetherPolicy::new(RolePermissionProvider::new(policy_repo));
            let role_service = crate::role::service::RoleServiceImpl::new(role_repo, role_policy);

            return role_service
                .delete_role(identity, organisation_id, role_id)
                .await;
        }

        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = tokio::sync::Mutex::new(Some(tx));

        let result = {
            let role_repo = PostgresRoleRepository::from_tx(&tx);
            let policy_repo = PostgresRoleRepository::from_pool(self.pool());
            let role_policy = AetherPolicy::new(RolePermissionProvider::new(policy_repo));
            let role_service = crate::role::service::RoleServiceImpl::new(role_repo, role_policy);

            role_service
                .delete_role(identity, organisation_id, role_id)
                .await
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

    async fn get_role(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        role_id: RoleId,
    ) -> Result<Option<Role>, CoreError> {
        #[cfg(feature = "test-mocks")]
        if let Some(role_repo) = crate::test_mocks::role_repository() {
            let policy_repo = role_repo.clone();
            let role_policy = AetherPolicy::new(RolePermissionProvider::new(policy_repo));
            let role_service = crate::role::service::RoleServiceImpl::new(role_repo, role_policy);

            return role_service
                .get_role(identity, organisation_id, role_id)
                .await;
        }

        let role_repo = PostgresRoleRepository::from_pool(self.pool());
        let policy_repo = PostgresRoleRepository::from_pool(self.pool());
        let role_policy = AetherPolicy::new(RolePermissionProvider::new(policy_repo));
        let role_service = crate::role::service::RoleServiceImpl::new(role_repo, role_policy);

        role_service
            .get_role(identity, organisation_id, role_id)
            .await
    }

    async fn list_roles_by_organisation(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Role>, CoreError> {
        #[cfg(feature = "test-mocks")]
        if let Some(role_repo) = crate::test_mocks::role_repository() {
            let policy_repo = role_repo.clone();
            let role_policy = AetherPolicy::new(RolePermissionProvider::new(policy_repo));
            let role_service = crate::role::service::RoleServiceImpl::new(role_repo, role_policy);

            return role_service
                .list_roles_by_organisation(identity, organisation_id)
                .await;
        }

        let role_repo = PostgresRoleRepository::from_pool(self.pool());
        let policy_repo = PostgresRoleRepository::from_pool(self.pool());
        let role_policy = AetherPolicy::new(RolePermissionProvider::new(policy_repo));
        let role_service = crate::role::service::RoleServiceImpl::new(role_repo, role_policy);

        role_service
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
        #[cfg(feature = "test-mocks")]
        if let Some(role_repo) = crate::test_mocks::role_repository() {
            let policy_repo = role_repo.clone();
            let role_policy = AetherPolicy::new(RolePermissionProvider::new(policy_repo));
            let role_service = crate::role::service::RoleServiceImpl::new(role_repo, role_policy);

            return role_service
                .update_role(identity, organisation_id, role_id, command)
                .await;
        }

        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = tokio::sync::Mutex::new(Some(tx));

        let result = {
            let role_repo = PostgresRoleRepository::from_tx(&tx);
            let policy_repo = PostgresRoleRepository::from_pool(self.pool());
            let role_policy = AetherPolicy::new(RolePermissionProvider::new(policy_repo));
            let role_service = crate::role::service::RoleServiceImpl::new(role_repo, role_policy);

            role_service
                .update_role(identity, organisation_id, role_id, command)
                .await
        };

        match result {
            Ok(role) => {
                super::take_transaction(&tx)
                    .await?
                    .commit()
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                Ok(role)
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
}
