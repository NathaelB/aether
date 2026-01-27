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

#[cfg(test)]
mod tests {
    use super::*;
    use aether_auth::{Identity, User};
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

    fn identity() -> Identity {
        Identity::User(User {
            id: "user-1".to_string(),
            username: "user".to_string(),
            email: None,
            name: None,
            roles: vec![],
        })
    }

    #[tokio::test]
    async fn create_role_maps_pool_error() {
        let command = CreateRoleCommand::new("admin".to_string(), 7)
            .with_organisation_id(OrganisationId(Uuid::new_v4()));

        let result = service().create_role(identity(), command).await;
        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn list_roles_rejects_permission() {
        let identity = Identity::User(User {
            id: "user-1".to_string(),
            username: "user".to_string(),
            email: None,
            name: None,
            roles: vec![],
        });

        let result = service()
            .list_roles_by_organisation(identity, OrganisationId(Uuid::new_v4()))
            .await;

        assert!(matches!(result, Err(CoreError::PermissionDenied { .. })));
    }

    #[tokio::test]
    async fn get_role_rejects_permission() {
        let identity = Identity::User(User {
            id: "user-1".to_string(),
            username: "user".to_string(),
            email: None,
            name: None,
            roles: vec![],
        });

        let result = service()
            .get_role(
                identity,
                OrganisationId(Uuid::new_v4()),
                RoleId(Uuid::new_v4()),
            )
            .await;

        assert!(matches!(result, Err(CoreError::PermissionDenied { .. })));
    }

    #[tokio::test]
    async fn delete_role_rejects_permission() {
        let identity = Identity::User(User {
            id: "user-1".to_string(),
            username: "user".to_string(),
            email: None,
            name: None,
            roles: vec![],
        });

        let result = service()
            .delete_role(
                identity,
                OrganisationId(Uuid::new_v4()),
                RoleId(Uuid::new_v4()),
            )
            .await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn update_role_rejects_permission() {
        let identity = Identity::User(User {
            id: "user-1".to_string(),
            username: "user".to_string(),
            email: None,
            name: None,
            roles: vec![],
        });

        let command = UpdateRoleCommand::new().with_name("viewer".to_string());
        let result = service()
            .update_role(
                identity,
                OrganisationId(Uuid::new_v4()),
                RoleId(Uuid::new_v4()),
                command,
            )
            .await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }
}
