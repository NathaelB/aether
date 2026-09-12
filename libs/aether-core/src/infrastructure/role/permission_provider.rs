use aether_auth::Identity;
use aether_permission::Permissions;

use crate::domain::{
    CoreError,
    organisation::{OrganisationId, ports::OrganisationRepository},
    role::{
        Role,
        ports::{PermissionProvider, RoleRepository},
    },
    user::ports::UserRepository,
};

#[derive(Clone)]
pub struct RolePermissionProvider<R, O, U>
where
    R: RoleRepository,
    O: OrganisationRepository,
    U: UserRepository,
{
    role_repository: R,
    organisation_repository: O,
    user_repository: U,
}

impl<R, O, U> RolePermissionProvider<R, O, U>
where
    R: RoleRepository,
    O: OrganisationRepository,
    U: UserRepository,
{
    pub fn new(role_repository: R, organisation_repository: O, user_repository: U) -> Self {
        Self {
            role_repository,
            organisation_repository,
            user_repository,
        }
    }

    /// How this caller stands to this organisation.
    ///
    /// One lookup for both rules, because both need the person behind the
    /// token. A caller the platform cannot resolve to a person, or an
    /// organisation that is not there, stands outside it: an agent
    /// authenticating as a client owns nothing and belongs to nothing until
    /// somebody puts it in.
    async fn standing(
        &self,
        identity: &Identity,
        organisation_id: OrganisationId,
    ) -> Result<Standing, CoreError> {
        let Some(user) = self.user_repository.find_by_sub(identity.id()).await? else {
            return Ok(Standing::Outside);
        };

        let Some(organisation) = self
            .organisation_repository
            .find_by_id(&organisation_id)
            .await?
        else {
            return Ok(Standing::Outside);
        };

        if organisation.owner_id == user.id {
            return Ok(Standing::Owner);
        }

        if self
            .organisation_repository
            .is_member(&organisation_id, &user.id)
            .await?
        {
            return Ok(Standing::Member);
        }

        Ok(Standing::Outside)
    }
}

/// Where a caller stands to one organisation.
///
/// Three states rather than a pair of booleans, so "neither owner nor member"
/// is a case the compiler makes you handle rather than the one you fall
/// through to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    Owner,
    Member,
    Outside,
}

impl<R, O, U> PermissionProvider for RolePermissionProvider<R, O, U>
where
    R: RoleRepository,
    O: OrganisationRepository,
    U: UserRepository,
{
    async fn permissions_for_organisation(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> Result<Permissions, CoreError> {
        match self.standing(&identity, organisation_id).await? {
            // The owner holds everything in their own organisation, without a
            // role saying so. Roles are how an owner hands parts of that out;
            // there is nobody above them to hand them theirs, and an
            // organisation whose owner can be locked out of it by deleting a
            // role is one nobody can recover.
            Standing::Owner => return Ok(Permissions::ADMINISTRATOR),
            Standing::Member => {}
            // Nothing below this line asks who the caller is inside this
            // organisation. Role names come from the token and are a
            // namespace shared by every tenant, so an organisation naming a
            // role `admin` used to hand its admin rights to anybody carrying
            // a realm role of that name, member or not. Belonging is what
            // makes the match mean anything.
            Standing::Outside => return Ok(Permissions::empty()),
        }

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
    use crate::domain::{
        organisation::{
            Organisation,
            commands::CreateOrganisationData,
            value_objects::{OrganisationName, OrganisationSlug, OrganisationStatus, Plan},
        },
        role::RoleId,
        user::{User, UserId},
    };
    use chrono::Utc;
    use uuid::Uuid;

    const ORGANISATION: Uuid = Uuid::from_u128(1);
    const OWNER: Uuid = Uuid::from_u128(2);
    const SOMEBODY_ELSE: Uuid = Uuid::from_u128(3);

    #[derive(Clone)]
    struct StubRoles(Vec<Role>);

    impl RoleRepository for StubRoles {
        async fn insert(&self, _role: Role) -> Result<(), CoreError> {
            unreachable!("a permission check never writes a role")
        }

        async fn get_by_id(&self, _role_id: RoleId) -> Result<Option<Role>, CoreError> {
            unreachable!("a permission check asks by name")
        }

        async fn list_by_organisation(
            &self,
            _organisation_id: OrganisationId,
        ) -> Result<Vec<Role>, CoreError> {
            unreachable!("a permission check asks by name")
        }

        async fn list_by_names(
            &self,
            _organisation_id: OrganisationId,
            names: Vec<String>,
        ) -> Result<Vec<Role>, CoreError> {
            Ok(self
                .0
                .iter()
                .filter(|role| names.contains(&role.name))
                .cloned()
                .collect())
        }

        async fn update(&self, _role: Role) -> Result<(), CoreError> {
            unreachable!("a permission check never writes a role")
        }

        async fn delete(&self, _role_id: RoleId) -> Result<(), CoreError> {
            unreachable!("a permission check never writes a role")
        }
    }

    #[derive(Clone)]
    struct StubOrganisations {
        organisation: Option<Organisation>,
        members: Vec<Uuid>,
    }

    impl StubOrganisations {
        fn owned_by(owner: Uuid) -> Self {
            Self {
                organisation: Some(organisation(owner)),
                members: Vec::new(),
            }
        }

        fn with_member(mut self, user: Uuid) -> Self {
            self.members.push(user);
            self
        }

        fn missing() -> Self {
            Self {
                organisation: None,
                members: Vec::new(),
            }
        }
    }

    impl OrganisationRepository for StubOrganisations {
        async fn find_by_id(
            &self,
            _id: &OrganisationId,
        ) -> Result<Option<Organisation>, CoreError> {
            Ok(self.organisation.clone())
        }

        async fn is_member(
            &self,
            _organisation_id: &OrganisationId,
            user_id: &UserId,
        ) -> Result<bool, CoreError> {
            Ok(self.members.contains(&user_id.0))
        }

        async fn create(&self, _data: CreateOrganisationData) -> Result<Organisation, CoreError> {
            unreachable!("a permission check never writes an organisation")
        }

        async fn insert_member(
            &self,
            _organisation_id: &OrganisationId,
            _user_id: &UserId,
        ) -> Result<(), CoreError> {
            unreachable!("a permission check never writes a membership")
        }

        async fn find_by_slug(
            &self,
            _slug: &OrganisationSlug,
        ) -> Result<Option<Organisation>, CoreError> {
            unreachable!("a permission check has the id")
        }

        async fn find_by_owner(&self, _owner_id: &UserId) -> Result<Vec<Organisation>, CoreError> {
            unreachable!("a permission check asks about one organisation")
        }

        async fn find_by_member(
            &self,
            _member_id: &UserId,
        ) -> Result<Vec<Organisation>, CoreError> {
            unreachable!("a permission check asks about one organisation")
        }

        async fn list(
            &self,
            _status: Option<OrganisationStatus>,
            _limit: usize,
            _offset: usize,
        ) -> Result<Vec<Organisation>, CoreError> {
            unreachable!("a permission check asks about one organisation")
        }

        async fn update(&self, _organisation: Organisation) -> Result<Organisation, CoreError> {
            unreachable!("a permission check never writes an organisation")
        }

        async fn delete(&self, _id: &OrganisationId) -> Result<(), CoreError> {
            unreachable!("a permission check never writes an organisation")
        }

        async fn slug_exists(&self, _slug: &OrganisationSlug) -> Result<bool, CoreError> {
            unreachable!("a permission check has the id")
        }

        async fn count(&self) -> Result<usize, CoreError> {
            unreachable!("a permission check asks about one organisation")
        }

        async fn count_by_status(&self, _status: OrganisationStatus) -> Result<usize, CoreError> {
            unreachable!("a permission check asks about one organisation")
        }
    }

    struct StubUsers(Option<User>);

    impl UserRepository for StubUsers {
        async fn upsert_by_email(&self, _user: &User) -> Result<User, CoreError> {
            unreachable!("a permission check never writes a user")
        }

        async fn find_by_sub(&self, _sub: &str) -> Result<Option<User>, CoreError> {
            // Rebuilt rather than cloned: `User` is not `Clone`, and the stub
            // only has to answer the one question the provider asks.
            Ok(self.0.as_ref().map(|held| User {
                id: held.id,
                email: held.email.clone(),
                name: held.name.clone(),
                sub: held.sub.clone(),
                created_at: held.created_at,
                updated_at: held.updated_at,
            }))
        }
    }

    fn user(id: Uuid) -> User {
        User {
            id: UserId(id),
            email: "someone@example.test".to_string(),
            name: "Someone".to_string(),
            sub: id.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn organisation(owner: Uuid) -> Organisation {
        let mut organisation = Organisation::new(
            OrganisationName::new("FerrisLabs").expect("a name"),
            OrganisationSlug::new("ferrislabs").expect("a slug"),
            UserId(owner),
            Plan::Free,
        );
        organisation.id = OrganisationId(ORGANISATION);
        organisation
    }

    fn role(name: &str, permissions: Permissions) -> Role {
        Role {
            id: RoleId(Uuid::new_v4()),
            name: name.to_string(),
            permissions: permissions.bits(),
            organisation_id: Some(OrganisationId(ORGANISATION)),
            color: None,
            created_at: Utc::now(),
        }
    }

    fn caller(sub: Uuid, roles: Vec<String>) -> Identity {
        Identity::User(aether_auth::User {
            id: sub.to_string(),
            username: "someone".to_string(),
            email: None,
            name: None,
            roles,
        })
    }

    fn provider(
        roles: Vec<Role>,
        organisations: StubOrganisations,
        caller: Option<User>,
    ) -> RolePermissionProvider<StubRoles, StubOrganisations, StubUsers> {
        RolePermissionProvider::new(StubRoles(roles), organisations, StubUsers(caller))
    }

    async fn permissions_of(
        provider: &RolePermissionProvider<StubRoles, StubOrganisations, StubUsers>,
        identity: Identity,
    ) -> Permissions {
        provider
            .permissions_for_organisation(identity, OrganisationId(ORGANISATION))
            .await
            .expect("answered")
    }

    /// The one this exists for. An owner holds no role in their own
    /// organisation, because there is nobody above them to have granted one.
    #[tokio::test]
    async fn the_owner_holds_everything_without_a_role() {
        let provider = provider(
            Vec::new(),
            StubOrganisations::owned_by(OWNER),
            Some(user(OWNER)),
        );

        let permissions = permissions_of(&provider, caller(OWNER, Vec::new())).await;

        assert!(permissions.can(Permissions::VIEW_INSTANCES));
        assert!(permissions.can(Permissions::MANAGE_ROLES));
        assert!(permissions.can(Permissions::READ_INSTANCE_LOGS));
    }

    /// The rule this file exists to enforce, and the one it did not.
    ///
    /// Role names come from the token and are a namespace every tenant
    /// shares. Two organisations both naming a role `admin` used to hand
    /// their admin rights to anybody carrying a realm role of that name --
    /// no membership, no ownership, nothing linking the caller to the
    /// organisation being asked about.
    #[tokio::test]
    async fn a_matching_role_name_grants_nothing_to_somebody_from_outside() {
        let provider = provider(
            vec![role("admin", Permissions::ADMINISTRATOR)],
            // Owned by somebody else, and the caller is in no member row.
            StubOrganisations::owned_by(OWNER),
            Some(user(SOMEBODY_ELSE)),
        );

        let permissions =
            permissions_of(&provider, caller(SOMEBODY_ELSE, vec!["admin".to_string()])).await;

        assert_eq!(permissions, Permissions::empty());
    }

    /// The other half of the same rule: belonging is what makes the role name
    /// mean something. The two tests differ by one member row.
    #[tokio::test]
    async fn the_same_role_name_grants_everything_it_says_to_a_member() {
        let provider = provider(
            vec![role("admin", Permissions::ADMINISTRATOR)],
            StubOrganisations::owned_by(OWNER).with_member(SOMEBODY_ELSE),
            Some(user(SOMEBODY_ELSE)),
        );

        let permissions =
            permissions_of(&provider, caller(SOMEBODY_ELSE, vec!["admin".to_string()])).await;

        assert!(permissions.can(Permissions::MANAGE_MEMBERS));
    }

    /// A service account is a caller like any other. It holds nothing in an
    /// organisation until somebody puts it in one, which is the whole point
    /// of not carving out a bypass for it.
    #[tokio::test]
    async fn a_caller_the_platform_cannot_place_holds_nothing() {
        let provider = provider(
            vec![role("admin", Permissions::ADMINISTRATOR)],
            StubOrganisations::owned_by(OWNER),
            None,
        );

        let permissions =
            permissions_of(&provider, caller(SOMEBODY_ELSE, vec!["admin".to_string()])).await;

        assert_eq!(permissions, Permissions::empty());
    }

    /// Everybody else gets what they were granted, and nothing more.
    #[tokio::test]
    async fn somebody_who_is_not_the_owner_gets_only_their_roles() {
        let provider = provider(
            vec![role("viewer", Permissions::VIEW_INSTANCES)],
            StubOrganisations::owned_by(OWNER).with_member(SOMEBODY_ELSE),
            Some(user(SOMEBODY_ELSE)),
        );

        let permissions =
            permissions_of(&provider, caller(SOMEBODY_ELSE, vec!["viewer".to_string()])).await;

        assert!(permissions.can(Permissions::VIEW_INSTANCES));
        assert!(!permissions.can(Permissions::MANAGE_ROLES));
    }

    #[tokio::test]
    async fn somebody_with_no_role_and_no_organisation_of_their_own_gets_nothing() {
        let provider = provider(
            Vec::new(),
            StubOrganisations::owned_by(OWNER).with_member(SOMEBODY_ELSE),
            Some(user(SOMEBODY_ELSE)),
        );

        let permissions = permissions_of(&provider, caller(SOMEBODY_ELSE, Vec::new())).await;

        assert!(permissions.is_empty());
    }

    /// A data plane agent authenticates as a client and owns nothing. Reading
    /// its subject as a person would be how a service account inherits an
    /// organisation it has no business in.
    #[tokio::test]
    async fn a_caller_who_is_not_a_person_owns_nothing() {
        let provider = provider(Vec::new(), StubOrganisations::owned_by(OWNER), None);

        let identity = Identity::Client(aether_auth::Client {
            id: OWNER.to_string(),
            client_id: "herald-service".to_string(),
            roles: vec![],
            scopes: vec![],
        });

        let permissions = permissions_of(&provider, identity).await;

        assert!(permissions.is_empty());
    }

    /// An organisation that is not there has no owner to be, so nothing is
    /// granted on the strength of a missing row.
    #[tokio::test]
    async fn an_organisation_that_is_not_there_grants_nothing() {
        let provider = provider(Vec::new(), StubOrganisations::missing(), Some(user(OWNER)));

        let permissions = permissions_of(&provider, caller(OWNER, Vec::new())).await;

        assert!(permissions.is_empty());
    }
}
