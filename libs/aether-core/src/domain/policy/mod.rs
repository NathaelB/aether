use aether_auth::Identity;
use aether_permission::Permissions;

use crate::domain::{
    CoreError,
    organisation::OrganisationId,
    role::ports::{PermissionProvider, RolePolicy},
};

#[derive(Debug, Clone, Copy)]
pub struct PolicyContext {
    permissions: Permissions,
}

impl PolicyContext {
    pub fn new(permissions: Permissions) -> Self {
        Self { permissions }
    }

    pub fn permissions(&self) -> Permissions {
        self.permissions
    }

    pub fn can(&self, permission: Permissions) -> bool {
        self.permissions.can(permission)
    }

    pub fn can_any(&self, permissions: &[Permissions]) -> bool {
        self.permissions.has_any(permissions)
    }

    pub fn can_all(&self, permissions: &[Permissions]) -> bool {
        permissions.iter().all(|&perm| self.permissions.can(perm))
    }

    pub fn require_permission(&self, permission: Permissions) -> Result<(), CoreError> {
        if self.can(permission) {
            Ok(())
        } else {
            Err(CoreError::PermissionDenied {
                reason: "insufficient permissions".to_string(),
            })
        }
    }

    pub fn require_any(&self, permissions: &[Permissions]) -> Result<(), CoreError> {
        if self.can_any(permissions) {
            Ok(())
        } else {
            Err(CoreError::PermissionDenied {
                reason: "insufficient permissions".to_string(),
            })
        }
    }

    pub fn require_all(&self, permissions: &[Permissions]) -> Result<(), CoreError> {
        if self.can_all(permissions) {
            Ok(())
        } else {
            Err(CoreError::PermissionDenied {
                reason: "insufficient permissions".to_string(),
            })
        }
    }
}

pub struct AetherPolicy<R>
where
    R: PermissionProvider,
{
    role_permission_provider: R,
}

impl<R> AetherPolicy<R>
where
    R: PermissionProvider,
{
    pub fn new(role_permission_provider: R) -> Self {
        Self {
            role_permission_provider,
        }
    }
}

impl<R> RolePolicy for AetherPolicy<R>
where
    R: PermissionProvider,
{
    async fn can_view_roles(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> Result<(), CoreError> {
        let permissions = self
            .role_permission_provider
            .permissions_for_organisation(identity, organisation_id)
            .await?;

        let context = PolicyContext::new(permissions);
        context.require_any(&[
            Permissions::ADMINISTRATOR,
            Permissions::VIEW_ROLES,
            Permissions::MANAGE_ROLES,
            Permissions::MANAGE_ORGANISATION,
        ])
    }

    async fn can_manage_roles(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> Result<(), CoreError> {
        let permissions = self
            .role_permission_provider
            .permissions_for_organisation(identity, organisation_id)
            .await?;

        let context = PolicyContext::new(permissions);
        context.require_any(&[
            Permissions::ADMINISTRATOR,
            Permissions::MANAGE_ROLES,
            Permissions::MANAGE_ORGANISATION,
        ])
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::domain::role::ports::MockPermissionProvider;
    use uuid::Uuid;

    #[test]
    fn policy_context_checks_permissions() {
        let perms = Permissions::VIEW_ROLES | Permissions::MANAGE_ROLES;
        let ctx = PolicyContext::new(perms);

        assert!(ctx.can(Permissions::VIEW_ROLES));
        assert!(ctx.can_any(&[Permissions::MANAGE_ROLES, Permissions::VIEW_MEMBERS]));
        assert!(!ctx.can_all(&[Permissions::VIEW_ROLES, Permissions::MANAGE_ORGANISATION]));
    }

    #[test]
    fn policy_context_require_helpers() {
        let ctx = PolicyContext::new(Permissions::VIEW_ROLES);

        assert!(ctx.require_permission(Permissions::VIEW_ROLES).is_ok());
        assert!(
            ctx.require_any(&[Permissions::MANAGE_ROLES, Permissions::VIEW_ROLES])
                .is_ok()
        );
        assert!(
            ctx.require_all(&[Permissions::VIEW_ROLES, Permissions::MANAGE_ROLES])
                .is_err()
        );
    }

    #[tokio::test]
    async fn aether_policy_allows_view_roles() {
        let mut provider = MockPermissionProvider::new();
        provider
            .expect_permissions_for_organisation()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(Permissions::VIEW_ROLES) }));

        let policy = AetherPolicy::new(provider);
        let identity = Identity::User(aether_auth::User {
            id: "user-1".to_string(),
            username: "user".to_string(),
            email: None,
            name: None,
            roles: vec![],
        });

        let result = policy
            .can_view_roles(identity, OrganisationId(Uuid::new_v4()))
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn aether_policy_allows_manage_roles_with_manage_permission() {
        let mut provider = MockPermissionProvider::new();
        provider
            .expect_permissions_for_organisation()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(Permissions::MANAGE_ROLES) }));

        let policy = AetherPolicy::new(provider);
        let identity = Identity::User(aether_auth::User {
            id: "user-1".to_string(),
            username: "user".to_string(),
            email: None,
            name: None,
            roles: vec![],
        });

        let result = policy
            .can_manage_roles(identity, OrganisationId(Uuid::new_v4()))
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn aether_policy_allows_view_roles_with_manage_permission() {
        let mut provider = MockPermissionProvider::new();
        provider
            .expect_permissions_for_organisation()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(Permissions::MANAGE_ROLES) }));

        let policy = AetherPolicy::new(provider);
        let identity = Identity::User(aether_auth::User {
            id: "user-1".to_string(),
            username: "user".to_string(),
            email: None,
            name: None,
            roles: vec![],
        });

        let result = policy
            .can_view_roles(identity, OrganisationId(Uuid::new_v4()))
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn aether_policy_denies_manage_roles() {
        let mut provider = MockPermissionProvider::new();
        provider
            .expect_permissions_for_organisation()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(Permissions::VIEW_ROLES) }));

        let policy = AetherPolicy::new(provider);
        let identity = Identity::User(aether_auth::User {
            id: "user-1".to_string(),
            username: "user".to_string(),
            email: None,
            name: None,
            roles: vec![],
        });

        let result = policy
            .can_manage_roles(identity, OrganisationId(Uuid::new_v4()))
            .await;

        assert!(result.is_err());
    }
}
