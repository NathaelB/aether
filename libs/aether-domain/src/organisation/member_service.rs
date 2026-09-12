//! Reading and changing who is in an organisation.

use aether_auth::Identity;

use crate::{
    CoreError,
    organisation::{
        OrganisationId,
        member::Member,
        ports::{MemberPolicy, MemberService, OrganisationRepository},
    },
    role::{RoleId, ports::RoleRepository},
    user::UserId,
};

pub struct MemberServiceImpl<O, R, P> {
    organisation_repository: O,
    role_repository: R,
    policy: P,
}

impl<O, R, P> MemberServiceImpl<O, R, P> {
    pub fn new(organisation_repository: O, role_repository: R, policy: P) -> Self {
        Self {
            organisation_repository,
            role_repository,
            policy,
        }
    }
}

impl<O, R, P> MemberServiceImpl<O, R, P>
where
    O: OrganisationRepository,
{
    async fn read(
        &self,
        organisation_id: OrganisationId,
        user_id: UserId,
    ) -> Result<Member, CoreError> {
        self.organisation_repository
            .find_member(&organisation_id, &user_id)
            .await?
            .ok_or(CoreError::MemberNotFound {
                organisation: organisation_id.0,
                user: user_id.0,
            })
    }

    async fn owns(
        &self,
        organisation_id: OrganisationId,
        user_id: UserId,
    ) -> Result<bool, CoreError> {
        let organisation = self
            .organisation_repository
            .find_by_id(&organisation_id)
            .await?
            .ok_or(CoreError::OrganisationNotFound {
                id: organisation_id.0,
            })?;

        Ok(organisation.owner_id == user_id)
    }
}

impl<O, R, P> MemberService for MemberServiceImpl<O, R, P>
where
    O: OrganisationRepository,
    R: RoleRepository,
    P: MemberPolicy,
{
    async fn list_members(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Member>, CoreError> {
        self.policy
            .can_view_members(identity, organisation_id)
            .await?;

        self.organisation_repository
            .list_members(&organisation_id)
            .await
    }

    async fn member(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        user_id: UserId,
    ) -> Result<Member, CoreError> {
        self.policy
            .can_view_members(identity, organisation_id)
            .await?;

        self.read(organisation_id, user_id).await
    }

    async fn set_member_roles(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        user_id: UserId,
        roles: Vec<RoleId>,
    ) -> Result<Member, CoreError> {
        self.policy
            .can_manage_members(identity, organisation_id)
            .await?;

        // Read first, so granting roles to somebody who is not in the
        // organisation fails as a missing member rather than as a write that
        // matched no rows.
        self.read(organisation_id, user_id).await?;

        // Every role has to be one of this organisation's. Granting one from
        // somewhere else would be the by-name matching this chantier removed,
        // wearing an id -- and the refusal names the role, because a caller
        // holding a list of ids needs to know which one was wrong.
        let known = self
            .role_repository
            .list_by_organisation(organisation_id)
            .await?;

        for role in &roles {
            if !known.iter().any(|known| known.id == *role) {
                return Err(CoreError::RoleNotInOrganisation {
                    organisation: organisation_id.0,
                    role: role.0,
                });
            }
        }

        self.organisation_repository
            .set_member_roles(&organisation_id, &user_id, &roles)
            .await?;

        self.read(organisation_id, user_id).await
    }

    async fn remove_member(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        user_id: UserId,
    ) -> Result<(), CoreError> {
        self.policy
            .can_remove_members(identity, organisation_id)
            .await?;

        // Refused rather than gated. There is no permission that makes this
        // possible, because an organisation without its owner is one nobody
        // can recover: the owner holds ADMINISTRATOR through ownership, and
        // nothing else in the model can hand that back.
        if self.owns(organisation_id, user_id).await? {
            return Err(CoreError::OwnerCannotBeRemoved {
                organisation: organisation_id.0,
            });
        }

        self.read(organisation_id, user_id).await?;

        self.organisation_repository
            .remove_member(&organisation_id, &user_id)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        organisation::{
            Organisation,
            commands::CreateOrganisationData,
            member::MemberId,
            value_objects::{
                OrganisationLimits, OrganisationName, OrganisationSlug, OrganisationStatus, Plan,
            },
        },
        role::Role,
    };
    use chrono::Utc;
    use uuid::Uuid;

    const ORGANISATION: Uuid = Uuid::from_u128(1);
    const OWNER: Uuid = Uuid::from_u128(2);
    const MEMBER: Uuid = Uuid::from_u128(3);
    const STRANGER: Uuid = Uuid::from_u128(4);
    const MINE: Uuid = Uuid::from_u128(5);
    const SOMEBODY_ELSES: Uuid = Uuid::from_u128(6);

    struct Allowed;

    impl MemberPolicy for Allowed {
        async fn can_view_members(&self, _: Identity, _: OrganisationId) -> Result<(), CoreError> {
            Ok(())
        }
        async fn can_manage_members(
            &self,
            _: Identity,
            _: OrganisationId,
        ) -> Result<(), CoreError> {
            Ok(())
        }
        async fn can_remove_members(
            &self,
            _: Identity,
            _: OrganisationId,
        ) -> Result<(), CoreError> {
            Ok(())
        }
    }

    struct Refused;

    impl MemberPolicy for Refused {
        async fn can_view_members(&self, _: Identity, _: OrganisationId) -> Result<(), CoreError> {
            Err(CoreError::PermissionDenied {
                reason: "no".to_string(),
            })
        }
        async fn can_manage_members(
            &self,
            _: Identity,
            _: OrganisationId,
        ) -> Result<(), CoreError> {
            Err(CoreError::PermissionDenied {
                reason: "no".to_string(),
            })
        }
        async fn can_remove_members(
            &self,
            _: Identity,
            _: OrganisationId,
        ) -> Result<(), CoreError> {
            Err(CoreError::PermissionDenied {
                reason: "no".to_string(),
            })
        }
    }

    fn role(id: Uuid, organisation: Uuid) -> Role {
        Role {
            id: RoleId(id),
            name: "role".to_string(),
            permissions: 4,
            organisation_id: Some(OrganisationId(organisation)),
            color: None,
            created_at: Utc::now(),
        }
    }

    fn organisation() -> Organisation {
        Organisation {
            id: OrganisationId(ORGANISATION),
            name: OrganisationName::new("acme".to_string()).expect("a name"),
            slug: OrganisationSlug::new("acme".to_string()).expect("a slug"),
            owner_id: UserId(OWNER),
            status: OrganisationStatus::Active,
            plan: Plan::Free,
            limits: OrganisationLimits::from_plan(&Plan::Free),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deleted_at: None,
        }
    }

    fn member(user: Uuid) -> Member {
        Member {
            id: MemberId(Uuid::from_u128(10)),
            organisation_id: OrganisationId(ORGANISATION),
            user_id: UserId(user),
            email: format!("{user}@acme.test"),
            name: "somebody".to_string(),
            roles: Vec::new(),
            joined_at: Utc::now(),
            invited_by: None,
        }
    }

    #[derive(Default, Clone)]
    struct StubOrganisations {
        members: Vec<Member>,
        removed: std::sync::Arc<std::sync::Mutex<Vec<Uuid>>>,
        granted: std::sync::Arc<std::sync::Mutex<Vec<RoleId>>>,
    }

    impl StubOrganisations {
        fn holding(members: Vec<Member>) -> Self {
            Self {
                members,
                ..Default::default()
            }
        }

        fn removed(&self) -> Vec<Uuid> {
            self.removed.lock().expect("not poisoned").clone()
        }

        fn granted(&self) -> Vec<RoleId> {
            self.granted.lock().expect("not poisoned").clone()
        }
    }

    impl OrganisationRepository for StubOrganisations {
        async fn find_by_id(&self, _: &OrganisationId) -> Result<Option<Organisation>, CoreError> {
            Ok(Some(organisation()))
        }

        async fn find_member(
            &self,
            _: &OrganisationId,
            user_id: &UserId,
        ) -> Result<Option<Member>, CoreError> {
            Ok(self
                .members
                .iter()
                .find(|member| member.user_id == *user_id)
                .cloned())
        }

        async fn list_members(&self, _: &OrganisationId) -> Result<Vec<Member>, CoreError> {
            Ok(self.members.clone())
        }

        async fn set_member_roles(
            &self,
            _: &OrganisationId,
            _: &UserId,
            roles: &[RoleId],
        ) -> Result<(), CoreError> {
            *self.granted.lock().expect("not poisoned") = roles.to_vec();
            Ok(())
        }

        async fn remove_member(
            &self,
            _: &OrganisationId,
            user_id: &UserId,
        ) -> Result<(), CoreError> {
            self.removed.lock().expect("not poisoned").push(user_id.0);
            Ok(())
        }

        async fn create(&self, _: CreateOrganisationData) -> Result<Organisation, CoreError> {
            unreachable!("members are not organisations")
        }
        async fn insert_member(&self, _: &OrganisationId, _: &UserId) -> Result<(), CoreError> {
            unreachable!("joining is an invitation, not a member edit")
        }
        async fn find_by_slug(
            &self,
            _: &OrganisationSlug,
        ) -> Result<Option<Organisation>, CoreError> {
            unreachable!("this service has the id")
        }
        async fn find_by_owner(&self, _: &UserId) -> Result<Vec<Organisation>, CoreError> {
            unreachable!("this service asks about one organisation")
        }
        async fn find_by_member(&self, _: &UserId) -> Result<Vec<Organisation>, CoreError> {
            unreachable!("this service asks about one organisation")
        }
        async fn list(
            &self,
            _: Option<OrganisationStatus>,
            _: usize,
            _: usize,
        ) -> Result<Vec<Organisation>, CoreError> {
            unreachable!("this service asks about one organisation")
        }
        async fn update(&self, _: Organisation) -> Result<Organisation, CoreError> {
            unreachable!("a member edit does not touch the organisation")
        }
        async fn delete(&self, _: &OrganisationId) -> Result<(), CoreError> {
            unreachable!("a member edit does not touch the organisation")
        }
        async fn slug_exists(&self, _: &OrganisationSlug) -> Result<bool, CoreError> {
            unreachable!("a member edit does not name an organisation")
        }
        async fn count(&self) -> Result<usize, CoreError> {
            unreachable!("this service asks about one organisation")
        }
        async fn count_by_status(&self, _: OrganisationStatus) -> Result<usize, CoreError> {
            unreachable!("this service asks about one organisation")
        }
    }

    #[derive(Clone)]
    struct StubRoles(Vec<Role>);

    impl RoleRepository for StubRoles {
        async fn list_by_organisation(&self, _: OrganisationId) -> Result<Vec<Role>, CoreError> {
            Ok(self.0.clone())
        }
        async fn insert(&self, _: Role) -> Result<(), CoreError> {
            unreachable!("granting does not create a role")
        }
        async fn get_by_id(&self, _: RoleId) -> Result<Option<Role>, CoreError> {
            unreachable!("the whole set is checked at once")
        }
        async fn list_by_names(
            &self,
            _: OrganisationId,
            _: Vec<String>,
        ) -> Result<Vec<Role>, CoreError> {
            unreachable!("names decide nothing any more")
        }
        async fn update(&self, _: Role) -> Result<(), CoreError> {
            unreachable!("granting does not edit a role")
        }
        async fn delete(&self, _: RoleId) -> Result<(), CoreError> {
            unreachable!("granting does not delete a role")
        }
    }

    fn caller() -> Identity {
        Identity::User(aether_auth::User {
            id: OWNER.to_string(),
            username: "somebody".to_string(),
            email: None,
            name: None,
            roles: Vec::new(),
        })
    }

    /// The rule with no permission behind it. An organisation without its
    /// owner is one nobody can recover, so this is refused rather than gated.
    #[tokio::test]
    async fn the_owner_cannot_be_removed_even_by_somebody_allowed_to_remove_members() {
        let organisations = StubOrganisations::holding(vec![member(OWNER)]);
        let service = MemberServiceImpl::new(organisations.clone(), StubRoles(Vec::new()), Allowed);

        let error = service
            .remove_member(caller(), OrganisationId(ORGANISATION), UserId(OWNER))
            .await
            .expect_err("the owner stays");

        assert!(matches!(error, CoreError::OwnerCannotBeRemoved { .. }));
        assert!(organisations.removed().is_empty());
    }

    /// And the message says why, rather than looking like a permission the
    /// caller might go and ask for.
    #[tokio::test]
    async fn the_refusal_says_it_is_about_the_owner() {
        let service = MemberServiceImpl::new(
            StubOrganisations::holding(vec![member(OWNER)]),
            StubRoles(Vec::new()),
            Allowed,
        );

        let message = service
            .remove_member(caller(), OrganisationId(ORGANISATION), UserId(OWNER))
            .await
            .expect_err("refused")
            .to_string();

        assert!(message.contains("owner"), "{message}");
    }

    #[tokio::test]
    async fn anybody_else_can_be_removed() {
        let organisations = StubOrganisations::holding(vec![member(OWNER), member(MEMBER)]);
        let service = MemberServiceImpl::new(organisations.clone(), StubRoles(Vec::new()), Allowed);

        service
            .remove_member(caller(), OrganisationId(ORGANISATION), UserId(MEMBER))
            .await
            .expect("removed");

        assert_eq!(organisations.removed(), vec![MEMBER]);
    }

    #[tokio::test]
    async fn removing_somebody_who_is_not_in_it_says_so() {
        let organisations = StubOrganisations::holding(vec![member(OWNER)]);
        let service = MemberServiceImpl::new(organisations.clone(), StubRoles(Vec::new()), Allowed);

        let error = service
            .remove_member(caller(), OrganisationId(ORGANISATION), UserId(STRANGER))
            .await
            .expect_err("not a member");

        assert!(matches!(error, CoreError::MemberNotFound { .. }));
        assert!(organisations.removed().is_empty());
    }

    /// A role belongs to one organisation. Granting one from somewhere else
    /// would be the by-name matching this chantier removed, wearing an id.
    #[tokio::test]
    async fn a_role_from_another_organisation_cannot_be_granted() {
        let organisations = StubOrganisations::holding(vec![member(MEMBER)]);
        let service = MemberServiceImpl::new(
            organisations.clone(),
            StubRoles(vec![role(MINE, ORGANISATION)]),
            Allowed,
        );

        let error = service
            .set_member_roles(
                caller(),
                OrganisationId(ORGANISATION),
                UserId(MEMBER),
                vec![RoleId(MINE), RoleId(SOMEBODY_ELSES)],
            )
            .await
            .expect_err("one of them is not ours");

        assert!(matches!(error, CoreError::RoleNotInOrganisation { .. }));
        // Nothing written: a partial grant would apply half of what was asked.
        assert!(organisations.granted().is_empty());
    }

    #[tokio::test]
    async fn the_refusal_names_the_role_that_was_wrong() {
        let service = MemberServiceImpl::new(
            StubOrganisations::holding(vec![member(MEMBER)]),
            StubRoles(vec![role(MINE, ORGANISATION)]),
            Allowed,
        );

        let message = service
            .set_member_roles(
                caller(),
                OrganisationId(ORGANISATION),
                UserId(MEMBER),
                vec![RoleId(SOMEBODY_ELSES)],
            )
            .await
            .expect_err("refused")
            .to_string();

        assert!(message.contains(&SOMEBODY_ELSES.to_string()), "{message}");
    }

    #[tokio::test]
    async fn roles_of_the_organisation_are_granted_whole() {
        let organisations = StubOrganisations::holding(vec![member(MEMBER)]);
        let service = MemberServiceImpl::new(
            organisations.clone(),
            StubRoles(vec![role(MINE, ORGANISATION)]),
            Allowed,
        );

        service
            .set_member_roles(
                caller(),
                OrganisationId(ORGANISATION),
                UserId(MEMBER),
                vec![RoleId(MINE)],
            )
            .await
            .expect("granted");

        assert_eq!(organisations.granted(), vec![RoleId(MINE)]);
    }

    /// Clearing the list is a state somebody chose: in the organisation,
    /// allowed nothing. It is not the same as being put out.
    #[tokio::test]
    async fn a_member_can_be_left_holding_no_roles() {
        let organisations = StubOrganisations::holding(vec![member(MEMBER)]);
        let service = MemberServiceImpl::new(organisations.clone(), StubRoles(Vec::new()), Allowed);

        service
            .set_member_roles(
                caller(),
                OrganisationId(ORGANISATION),
                UserId(MEMBER),
                Vec::new(),
            )
            .await
            .expect("granted nothing");

        assert!(organisations.granted().is_empty());
        assert!(organisations.removed().is_empty());
    }

    #[tokio::test]
    async fn nothing_is_read_or_written_without_the_right_to() {
        let organisations = StubOrganisations::holding(vec![member(MEMBER)]);
        let service = MemberServiceImpl::new(organisations.clone(), StubRoles(Vec::new()), Refused);

        assert!(
            service
                .list_members(caller(), OrganisationId(ORGANISATION))
                .await
                .is_err()
        );
        assert!(
            service
                .remove_member(caller(), OrganisationId(ORGANISATION), UserId(MEMBER))
                .await
                .is_err()
        );
        assert!(organisations.removed().is_empty());
    }
}
