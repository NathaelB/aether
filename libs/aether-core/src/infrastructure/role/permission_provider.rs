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
    role_repository: R,
}

impl<R> RolePermissionProvider<R>
where
    R: RoleRepository,
{
    pub fn new(role_repository: R) -> Self {
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
