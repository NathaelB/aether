use std::sync::Arc;

use aether_auth::Identity;
use aether_permission::Permissions;

use crate::domain::{
    CoreError,
    organisation::OrganisationId,
    role::{
        Role,
        ports::{PermissionProvider, RoleRepository},
    },
};

#[derive(Clone)]
pub struct RolePermissionProvider<R>
where
    R: RoleRepository,
{
    role_repository: Arc<R>,
}

impl<R> RolePermissionProvider<R>
where
    R: RoleRepository,
{
    pub fn new(role_repository: Arc<R>) -> Self {
        Self { role_repository }
    }
}

impl<R> PermissionProvider for RolePermissionProvider<R>
where
    R: RoleRepository,
{
    async fn permissions_for_organisation(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> Result<Permissions, CoreError> {
        let role_names = identity.roles().to_vec();
        if role_names.is_empty() {
            return Ok(Permissions::empty());
        }

        let roles = self
            .role_repository
            .list_by_names(organisation_id, role_names)
            .await?;

        Ok(permissions_from_roles(&roles))
    }
}

fn permissions_from_roles(roles: &[Role]) -> Permissions {
    let permissions = roles
        .iter()
        .map(|role| Permissions::from_bits_truncate(role.permissions))
        .collect::<Vec<_>>();

    Permissions::union_all(&permissions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::role::ports::MockRoleRepository;
    use uuid::Uuid;

    #[tokio::test]
    async fn permissions_from_roles_unions_flags() {
        let mut mock_repo = MockRoleRepository::new();
        let organisation_id = OrganisationId(Uuid::new_v4());

        mock_repo.expect_list_by_names().times(1).returning(|_, _| {
            let role1 = Role {
                id: crate::domain::role::RoleId(Uuid::new_v4()),
                name: "viewer".to_string(),
                permissions: Permissions::VIEW_ROLES.bits(),
                organisation_id: None,
                color: None,
                created_at: chrono::Utc::now(),
            };
            let role2 = Role {
                id: crate::domain::role::RoleId(Uuid::new_v4()),
                name: "manager".to_string(),
                permissions: Permissions::MANAGE_ROLES.bits(),
                organisation_id: None,
                color: None,
                created_at: chrono::Utc::now(),
            };
            Box::pin(async move { Ok(vec![role1, role2]) })
        });

        let provider = RolePermissionProvider::new(Arc::new(mock_repo));
        let identity = Identity::User(aether_auth::User {
            id: "user-1".to_string(),
            username: "user".to_string(),
            email: None,
            name: None,
            roles: vec!["viewer".to_string(), "manager".to_string()],
        });

        let perms = provider
            .permissions_for_organisation(identity, organisation_id)
            .await
            .unwrap();

        assert!(perms.can(Permissions::VIEW_ROLES));
        assert!(perms.can(Permissions::MANAGE_ROLES));
    }
}
