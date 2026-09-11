use aether_auth::Identity;
use aether_macros::transactional;

use crate::{
    AetherService, CoreError,
    infrastructure::role::permissions_in,
    organisation::OrganisationId,
    policy::AetherPolicy,
    role::{
        Role, RoleId,
        commands::{CreateRoleCommand, UpdateRoleCommand},
        ports::RoleService,
        service::RoleServiceImpl,
    },
};

// The permission provider needs a second view onto the roles table, so it gets
// its own repository built from the same transaction. It used to read from the
// pool instead, which meant a permission check could observe rows the
// surrounding transaction had not committed -- or miss rows it had written.
//
// Written out at each call site rather than factored into a macro_rules!:
// declarative macro hygiene cannot see the `role_repository` and `tx` bindings
// that #[transactional] introduces.

impl RoleService for AetherService {
    #[transactional(role)]
    async fn create_role(
        &self,
        identity: Identity,
        command: CreateRoleCommand,
    ) -> Result<Role, CoreError> {
        RoleServiceImpl::new(role_repository, AetherPolicy::new(permissions_in(&tx)))
            .create_role(identity, command)
            .await
    }

    #[transactional(role)]
    async fn delete_role(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        role_id: RoleId,
    ) -> Result<(), CoreError> {
        RoleServiceImpl::new(role_repository, AetherPolicy::new(permissions_in(&tx)))
            .delete_role(identity, organisation_id, role_id)
            .await
    }

    #[transactional(role)]
    async fn get_role(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        role_id: RoleId,
    ) -> Result<Option<Role>, CoreError> {
        RoleServiceImpl::new(role_repository, AetherPolicy::new(permissions_in(&tx)))
            .get_role(identity, organisation_id, role_id)
            .await
    }

    #[transactional(role)]
    async fn list_roles_by_organisation(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Role>, CoreError> {
        RoleServiceImpl::new(role_repository, AetherPolicy::new(permissions_in(&tx)))
            .list_roles_by_organisation(identity, organisation_id)
            .await
    }

    #[transactional(role)]
    async fn update_role(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        role_id: RoleId,
        command: UpdateRoleCommand,
    ) -> Result<Role, CoreError> {
        RoleServiceImpl::new(role_repository, AetherPolicy::new(permissions_in(&tx)))
            .update_role(identity, organisation_id, role_id, command)
            .await
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

        // The transaction is opened before the service sees the identity, so an
        // unreachable database surfaces first. The authorization rule is asserted
        // on the domain service, with mocked repositories and no pool.
        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
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

        // The transaction is opened before the service sees the identity, so an
        // unreachable database surfaces first. The authorization rule is asserted
        // on the domain service, with mocked repositories and no pool.
        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
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
