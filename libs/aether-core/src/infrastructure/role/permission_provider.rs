use aether_auth::Identity;
use aether_permission::Permissions;

use crate::domain::{
    CoreError,
    organisation::{OrganisationId, member::Member, ports::OrganisationRepository},
    role::ports::PermissionProvider,
    user::ports::UserRepository,
};

#[derive(Clone)]
/// Resolves what a caller may do in one organisation.
///
/// It holds no role repository. Roles used to be looked up by the names in
/// the token; they arrive with the membership now, so the lookup and the
/// dependency both went.
pub struct RolePermissionProvider<O, U>
where
    O: OrganisationRepository,
    U: UserRepository,
{
    organisation_repository: O,
    user_repository: U,
}

impl<O, U> RolePermissionProvider<O, U>
where
    O: OrganisationRepository,
    U: UserRepository,
{
    pub fn new(organisation_repository: O, user_repository: U) -> Self {
        Self {
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

        match self
            .organisation_repository
            .find_member(&organisation_id, &user.id)
            .await?
        {
            Some(member) => Ok(Standing::Member(member)),
            None => Ok(Standing::Outside),
        }
    }
}

/// Where a caller stands to one organisation.
///
/// Three states rather than a pair of booleans, so "neither owner nor member"
/// is a case the compiler makes you handle rather than the one you fall
/// through to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Standing {
    Owner,
    Member(Member),
    Outside,
}

impl<O, U> PermissionProvider for RolePermissionProvider<O, U>
where
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
            Standing::Owner => Ok(Permissions::ADMINISTRATOR),

            // Read from the membership, not from the token. The token names
            // realm roles, which are a namespace every tenant shares; what
            // somebody may do here is held here, by role id, granted by
            // somebody who could grant it.
            //
            // It also means a change takes effect on the next request rather
            // than on the next token: revoking a role no longer waits for
            // whatever the identity provider decided the lifetime should be.
            Standing::Member(member) => Ok(Permissions::from_bits_truncate(member.permissions())),

            Standing::Outside => Ok(Permissions::empty()),
        }
    }
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
        role::{Role, RoleId},
        user::{User, UserId},
    };
    use chrono::Utc;
    use uuid::Uuid;

    const ORGANISATION: Uuid = Uuid::from_u128(1);
    const OWNER: Uuid = Uuid::from_u128(2);
    const SOMEBODY_ELSE: Uuid = Uuid::from_u128(3);

    #[derive(Clone)]
    struct StubOrganisations {
        organisation: Option<Organisation>,
        members: Vec<(Uuid, Vec<Role>)>,
    }

    impl StubOrganisations {
        fn owned_by(owner: Uuid) -> Self {
            Self {
                organisation: Some(organisation(owner)),
                members: Vec::new(),
            }
        }

        /// In the organisation, granted nothing. The state that has to stay
        /// distinguishable from not being in it at all.
        fn with_member(self, user: Uuid) -> Self {
            self.with_member_holding(user, Vec::new())
        }

        fn with_member_holding(mut self, user: Uuid, roles: Vec<Role>) -> Self {
            self.members.push((user, roles));
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

        async fn find_member(
            &self,
            organisation_id: &OrganisationId,
            user_id: &UserId,
        ) -> Result<Option<Member>, CoreError> {
            Ok(self
                .members
                .iter()
                .find(|(id, _)| *id == user_id.0)
                .map(|(_, roles)| Member {
                    id: crate::domain::organisation::member::MemberId(Uuid::from_u128(99)),
                    organisation_id: *organisation_id,
                    user_id: *user_id,
                    email: "somebody@acme.test".to_string(),
                    name: "somebody".to_string(),
                    roles: roles.clone(),
                    joined_at: Utc::now(),
                    invited_by: None,
                }))
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

        async fn list_members(&self, _: &OrganisationId) -> Result<Vec<Member>, CoreError> {
            unreachable!("a permission check asks about one caller")
        }

        async fn set_member_roles(
            &self,
            _: &OrganisationId,
            _: &UserId,
            _: &[crate::domain::role::RoleId],
        ) -> Result<(), CoreError> {
            unreachable!("a permission check never grants anything")
        }

        async fn remove_member(&self, _: &OrganisationId, _: &UserId) -> Result<(), CoreError> {
            unreachable!("a permission check never removes anybody")
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

        async fn find_by_email(&self, _email: &str) -> Result<Option<User>, CoreError> {
            unreachable!("this suite does not look anybody up by address")
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
        organisations: StubOrganisations,
        caller: Option<User>,
    ) -> RolePermissionProvider<StubOrganisations, StubUsers> {
        RolePermissionProvider::new(organisations, StubUsers(caller))
    }

    async fn permissions_of(
        provider: &RolePermissionProvider<StubOrganisations, StubUsers>,
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
        let provider = provider(StubOrganisations::owned_by(OWNER), Some(user(OWNER)));

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
            // Owned by somebody else, and the caller is in no member row.
            StubOrganisations::owned_by(OWNER),
            Some(user(SOMEBODY_ELSE)),
        );

        let permissions =
            permissions_of(&provider, caller(SOMEBODY_ELSE, vec!["admin".to_string()])).await;

        assert_eq!(permissions, Permissions::empty());
    }

    /// What the chantier turns on. The token names realm roles, and realm
    /// roles are a namespace every tenant shares -- so they name nothing
    /// here. A member holds what their membership was granted, and a token
    /// claiming otherwise changes nothing either way.
    #[tokio::test]
    async fn what_the_token_calls_itself_does_not_decide_anything() {
        let granted = role("viewer", Permissions::VIEW_INSTANCES);
        let provider = provider(
            StubOrganisations::owned_by(OWNER).with_member_holding(SOMEBODY_ELSE, vec![granted]),
            Some(user(SOMEBODY_ELSE)),
        );

        // A token shouting `admin`, at a member granted `viewer`.
        let permissions =
            permissions_of(&provider, caller(SOMEBODY_ELSE, vec!["admin".to_string()])).await;

        assert!(permissions.can(Permissions::VIEW_INSTANCES));
        assert!(!permissions.can(Permissions::MANAGE_MEMBERS));
    }

    /// A member granted nothing is in the organisation and may do nothing in
    /// it. Distinct from not being in it, and the two are refused the same
    /// way today -- but only one of them is somebody waiting for a role.
    #[tokio::test]
    async fn a_member_granted_nothing_holds_nothing() {
        let provider = provider(
            StubOrganisations::owned_by(OWNER).with_member(SOMEBODY_ELSE),
            Some(user(SOMEBODY_ELSE)),
        );

        let permissions = permissions_of(&provider, caller(SOMEBODY_ELSE, Vec::new())).await;

        assert_eq!(permissions, Permissions::empty());
    }

    /// Two roles on one membership add up. An organisation may grant the same
    /// person several, and holding both is holding the union.
    #[tokio::test]
    async fn a_member_holds_everything_their_roles_add_up_to() {
        let provider = provider(
            StubOrganisations::owned_by(OWNER).with_member_holding(
                SOMEBODY_ELSE,
                vec![
                    role("viewer", Permissions::VIEW_INSTANCES),
                    role("operator", Permissions::UPGRADE_INSTANCES),
                ],
            ),
            Some(user(SOMEBODY_ELSE)),
        );

        let permissions = permissions_of(&provider, caller(SOMEBODY_ELSE, Vec::new())).await;

        assert!(permissions.can(Permissions::VIEW_INSTANCES));
        assert!(permissions.can(Permissions::UPGRADE_INSTANCES));
        assert!(!permissions.can(Permissions::MANAGE_MEMBERS));
    }

    /// The other half of the same rule: belonging is what makes the role name
    /// mean something. The two tests differ by one member row.
    #[tokio::test]
    async fn the_same_role_name_grants_everything_it_says_to_a_member() {
        let granted = role("admin", Permissions::ADMINISTRATOR);
        let provider = provider(
            StubOrganisations::owned_by(OWNER).with_member_holding(SOMEBODY_ELSE, vec![granted]),
            Some(user(SOMEBODY_ELSE)),
        );

        let permissions = permissions_of(&provider, caller(SOMEBODY_ELSE, Vec::new())).await;

        assert!(permissions.can(Permissions::MANAGE_MEMBERS));
    }

    /// A service account is a caller like any other. It holds nothing in an
    /// organisation until somebody puts it in one, which is the whole point
    /// of not carving out a bypass for it.
    #[tokio::test]
    async fn a_caller_the_platform_cannot_place_holds_nothing() {
        let provider = provider(StubOrganisations::owned_by(OWNER), None);

        let permissions =
            permissions_of(&provider, caller(SOMEBODY_ELSE, vec!["admin".to_string()])).await;

        assert_eq!(permissions, Permissions::empty());
    }

    /// Everybody else gets what they were granted, and nothing more.
    #[tokio::test]
    async fn somebody_who_is_not_the_owner_gets_only_their_roles() {
        let provider = provider(
            StubOrganisations::owned_by(OWNER).with_member_holding(
                SOMEBODY_ELSE,
                vec![role("viewer", Permissions::VIEW_INSTANCES)],
            ),
            Some(user(SOMEBODY_ELSE)),
        );

        let permissions = permissions_of(&provider, caller(SOMEBODY_ELSE, Vec::new())).await;

        assert!(permissions.can(Permissions::VIEW_INSTANCES));
        assert!(!permissions.can(Permissions::MANAGE_ROLES));
    }

    #[tokio::test]
    async fn somebody_with_no_role_and_no_organisation_of_their_own_gets_nothing() {
        let provider = provider(
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
        let provider = provider(StubOrganisations::owned_by(OWNER), None);

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
        let provider = provider(StubOrganisations::missing(), Some(user(OWNER)));

        let permissions = permissions_of(&provider, caller(OWNER, Vec::new())).await;

        assert!(permissions.is_empty());
    }
}
