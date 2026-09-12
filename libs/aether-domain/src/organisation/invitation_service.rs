//! Issuing, revoking and walking through invitations.

use aether_auth::Identity;
use chrono::Utc;

use crate::{
    CoreError,
    organisation::{
        OrganisationId,
        invitation::{Invitation, InvitationId, InvitationToken, InvitedEmail},
        member::Member,
        ports::{
            InvitationPolicy, InvitationRepository, InvitationService, OrganisationRepository,
        },
    },
    role::{RoleId, ports::RoleRepository},
    user::{UserId, ports::UserRepository},
};

pub struct InvitationServiceImpl<O, I, R, U, P> {
    organisation_repository: O,
    invitation_repository: I,
    role_repository: R,
    user_repository: U,
    policy: P,
}

impl<O, I, R, U, P> InvitationServiceImpl<O, I, R, U, P> {
    pub fn new(
        organisation_repository: O,
        invitation_repository: I,
        role_repository: R,
        user_repository: U,
        policy: P,
    ) -> Self {
        Self {
            organisation_repository,
            invitation_repository,
            role_repository,
            user_repository,
            policy,
        }
    }
}

impl<O, I, R, U, P> InvitationService for InvitationServiceImpl<O, I, R, U, P>
where
    O: OrganisationRepository,
    I: InvitationRepository,
    R: RoleRepository,
    U: UserRepository,
    P: InvitationPolicy,
{
    async fn invite(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        email: InvitedEmail,
        roles: Vec<RoleId>,
    ) -> Result<(Invitation, InvitationToken), CoreError> {
        self.policy
            .can_invite_members(identity.clone(), organisation_id)
            .await?;

        // Every role has to be one of this organisation's, the same rule
        // granting one to an existing member follows. An invitation that
        // promised a role from somewhere else would apply it on acceptance,
        // which is the cross-organisation grant by another route.
        let known = self
            .role_repository
            .list_by_organisation(organisation_id)
            .await?;

        let mut granted = Vec::with_capacity(roles.len());
        for role_id in &roles {
            let role = known.iter().find(|known| known.id == *role_id).ok_or(
                CoreError::RoleNotInOrganisation {
                    organisation: organisation_id.0,
                    role: role_id.0,
                },
            )?;

            granted.push(role.clone());
        }

        // Somebody already in does not need a way in, and issuing one anyway
        // would leave a live link that does nothing if used and looks
        // outstanding to whoever reads the list.
        if let Some(user) = self.user_repository.find_by_email(email.as_str()).await?
            && self
                .organisation_repository
                .find_member(&organisation_id, &user.id)
                .await?
                .is_some()
        {
            return Err(CoreError::AlreadyAMember {
                email: email.to_string(),
            });
        }

        let invited_by = self
            .user_repository
            .find_by_sub(identity.id())
            .await?
            .map(|user| user.id);

        let (invitation, token) =
            Invitation::issue(organisation_id, email, granted, invited_by, Utc::now());

        self.invitation_repository
            .save_invitation(&invitation, &token.hash())
            .await?;

        Ok((invitation, token))
    }

    async fn list_invitations(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Invitation>, CoreError> {
        self.policy
            .can_view_invitations(identity, organisation_id)
            .await?;

        self.invitation_repository
            .list_invitations(&organisation_id)
            .await
    }

    async fn revoke_invitation(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        invitation_id: InvitationId,
    ) -> Result<Invitation, CoreError> {
        self.policy
            .can_invite_members(identity, organisation_id)
            .await?;

        let mut invitation = self
            .invitation_repository
            .find_invitation(&organisation_id, &invitation_id)
            .await?
            .ok_or(CoreError::InvitationNotFound)?;

        let now = Utc::now();

        // Revoking one already accepted cuts the link and leaves the
        // membership alone. Putting somebody out is a different act, and it
        // needs a different right.
        if invitation.revoked_at.is_none() {
            self.invitation_repository
                .mark_invitation_revoked(&invitation_id, now)
                .await?;
            invitation.revoked_at = Some(now);
        }

        Ok(invitation)
    }

    async fn accept_invitation(
        &self,
        identity: Identity,
        token: InvitationToken,
    ) -> Result<Member, CoreError> {
        let invitation = self
            .invitation_repository
            .find_invitation_by_hash(&token.hash())
            .await?
            .ok_or(CoreError::InvitationNotFound)?;

        let now = Utc::now();
        invitation.claimable(now)?;

        // The caller has to be the person it was written to. Without this the
        // link is the credential, and a message forwarded by mistake becomes
        // an account in somebody else's organisation.
        let user = self
            .user_repository
            .find_by_sub(identity.id())
            .await?
            .ok_or(CoreError::InvitationAddressedToSomebodyElse)?;

        let holder = InvitedEmail::parse(&user.email)?;
        if holder != invitation.email {
            return Err(CoreError::InvitationAddressedToSomebodyElse);
        }

        self.join(&invitation, user.id).await?;

        self.invitation_repository
            .mark_invitation_accepted(&invitation.id, now)
            .await?;

        self.organisation_repository
            .find_member(&invitation.organisation_id, &user.id)
            .await?
            .ok_or(CoreError::MemberNotFound {
                organisation: invitation.organisation_id.0,
                user: user.id.0,
            })
    }
}

impl<O, I, R, U, P> InvitationServiceImpl<O, I, R, U, P>
where
    O: OrganisationRepository,
{
    /// Puts somebody in with what the invitation promised.
    ///
    /// Tolerates being already a member: the membership row is what matters,
    /// and an invitation issued before somebody joined another way should
    /// still hand them the roles it promised rather than fail.
    async fn join(&self, invitation: &Invitation, user_id: UserId) -> Result<(), CoreError> {
        if self
            .organisation_repository
            .find_member(&invitation.organisation_id, &user_id)
            .await?
            .is_none()
        {
            self.organisation_repository
                .insert_member(&invitation.organisation_id, &user_id)
                .await?;
        }

        self.organisation_repository
            .set_member_roles(
                &invitation.organisation_id,
                &user_id,
                &invitation.role_ids(),
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        organisation::{
            invitation::{InvitationState, InvitationTokenHash},
            member::MemberId,
        },
        role::Role,
        user::User,
    };
    use chrono::{DateTime, Duration};
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    const ORGANISATION: Uuid = Uuid::from_u128(1);
    const INVITER: Uuid = Uuid::from_u128(2);
    const NEWCOMER: Uuid = Uuid::from_u128(3);
    const MINE: Uuid = Uuid::from_u128(5);
    const SOMEBODY_ELSES: Uuid = Uuid::from_u128(6);

    struct Allowed;

    impl InvitationPolicy for Allowed {
        async fn can_view_invitations(
            &self,
            _: Identity,
            _: OrganisationId,
        ) -> Result<(), CoreError> {
            Ok(())
        }
        async fn can_invite_members(
            &self,
            _: Identity,
            _: OrganisationId,
        ) -> Result<(), CoreError> {
            Ok(())
        }
    }

    #[derive(Default, Clone)]
    struct Store {
        invitations: Arc<Mutex<Vec<(Invitation, InvitationTokenHash)>>>,
        members: Arc<Mutex<Vec<Uuid>>>,
        granted: Arc<Mutex<Vec<RoleId>>>,
    }

    impl Store {
        fn members(&self) -> Vec<Uuid> {
            self.members.lock().expect("not poisoned").clone()
        }
        fn granted(&self) -> Vec<RoleId> {
            self.granted.lock().expect("not poisoned").clone()
        }
        fn put(&self, invitation: Invitation, hash: InvitationTokenHash) {
            self.invitations
                .lock()
                .expect("not poisoned")
                .push((invitation, hash));
        }
    }

    impl InvitationRepository for Store {
        async fn save_invitation(
            &self,
            invitation: &Invitation,
            token_hash: &InvitationTokenHash,
        ) -> Result<(), CoreError> {
            self.put(invitation.clone(), token_hash.clone());
            Ok(())
        }

        async fn list_invitations(&self, _: &OrganisationId) -> Result<Vec<Invitation>, CoreError> {
            Ok(self
                .invitations
                .lock()
                .expect("not poisoned")
                .iter()
                .map(|(invitation, _)| invitation.clone())
                .collect())
        }

        async fn find_invitation_by_hash(
            &self,
            token_hash: &InvitationTokenHash,
        ) -> Result<Option<Invitation>, CoreError> {
            Ok(self
                .invitations
                .lock()
                .expect("not poisoned")
                .iter()
                .find(|(_, hash)| hash == token_hash)
                .map(|(invitation, _)| invitation.clone()))
        }

        async fn find_invitation(
            &self,
            _: &OrganisationId,
            invitation_id: &InvitationId,
        ) -> Result<Option<Invitation>, CoreError> {
            Ok(self
                .invitations
                .lock()
                .expect("not poisoned")
                .iter()
                .find(|(invitation, _)| invitation.id == *invitation_id)
                .map(|(invitation, _)| invitation.clone()))
        }

        async fn mark_invitation_accepted(
            &self,
            invitation_id: &InvitationId,
            at: DateTime<Utc>,
        ) -> Result<(), CoreError> {
            for (invitation, _) in self.invitations.lock().expect("not poisoned").iter_mut() {
                if invitation.id == *invitation_id && invitation.accepted_at.is_none() {
                    invitation.accepted_at = Some(at);
                }
            }
            Ok(())
        }

        async fn mark_invitation_revoked(
            &self,
            invitation_id: &InvitationId,
            at: DateTime<Utc>,
        ) -> Result<(), CoreError> {
            for (invitation, _) in self.invitations.lock().expect("not poisoned").iter_mut() {
                if invitation.id == *invitation_id {
                    invitation.revoked_at = Some(at);
                }
            }
            Ok(())
        }
    }

    fn role(id: Uuid) -> Role {
        Role {
            id: RoleId(id),
            name: "viewer".to_string(),
            permissions: 4,
            organisation_id: Some(OrganisationId(ORGANISATION)),
            color: None,
            created_at: Utc::now(),
        }
    }

    fn user(id: Uuid, email: &str) -> User {
        User {
            id: UserId(id),
            email: email.to_string(),
            name: "somebody".to_string(),
            sub: id.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn caller(sub: Uuid) -> Identity {
        Identity::User(aether_auth::User {
            id: sub.to_string(),
            username: "somebody".to_string(),
            email: None,
            name: None,
            roles: Vec::new(),
        })
    }

    #[derive(Clone)]
    struct Organisations(Store);

    impl OrganisationRepository for Organisations {
        async fn find_member(
            &self,
            organisation_id: &OrganisationId,
            user_id: &UserId,
        ) -> Result<Option<Member>, CoreError> {
            Ok(self.0.members().contains(&user_id.0).then(|| Member {
                id: MemberId(Uuid::from_u128(50)),
                organisation_id: *organisation_id,
                user_id: *user_id,
                email: "somebody@acme.test".to_string(),
                name: "somebody".to_string(),
                roles: Vec::new(),
                joined_at: Utc::now(),
                invited_by: None,
            }))
        }

        async fn insert_member(
            &self,
            _: &OrganisationId,
            user_id: &UserId,
        ) -> Result<(), CoreError> {
            self.0.members.lock().expect("not poisoned").push(user_id.0);
            Ok(())
        }

        async fn set_member_roles(
            &self,
            _: &OrganisationId,
            _: &UserId,
            roles: &[RoleId],
        ) -> Result<(), CoreError> {
            *self.0.granted.lock().expect("not poisoned") = roles.to_vec();
            Ok(())
        }

        async fn find_by_id(
            &self,
            _: &OrganisationId,
        ) -> Result<Option<crate::organisation::Organisation>, CoreError> {
            unreachable!("an invitation does not read the organisation")
        }
        async fn list_members(&self, _: &OrganisationId) -> Result<Vec<Member>, CoreError> {
            unreachable!("an invitation asks about one person")
        }
        async fn remove_member(&self, _: &OrganisationId, _: &UserId) -> Result<(), CoreError> {
            unreachable!("an invitation never removes anybody")
        }
        async fn create(
            &self,
            _: crate::organisation::commands::CreateOrganisationData,
        ) -> Result<crate::organisation::Organisation, CoreError> {
            unreachable!()
        }
        async fn find_by_slug(
            &self,
            _: &crate::organisation::value_objects::OrganisationSlug,
        ) -> Result<Option<crate::organisation::Organisation>, CoreError> {
            unreachable!()
        }
        async fn find_by_owner(
            &self,
            _: &UserId,
        ) -> Result<Vec<crate::organisation::Organisation>, CoreError> {
            unreachable!()
        }
        async fn find_by_member(
            &self,
            _: &UserId,
        ) -> Result<Vec<crate::organisation::Organisation>, CoreError> {
            unreachable!()
        }
        async fn list(
            &self,
            _: Option<crate::organisation::value_objects::OrganisationStatus>,
            _: usize,
            _: usize,
        ) -> Result<Vec<crate::organisation::Organisation>, CoreError> {
            unreachable!()
        }
        async fn update(
            &self,
            _: crate::organisation::Organisation,
        ) -> Result<crate::organisation::Organisation, CoreError> {
            unreachable!()
        }
        async fn delete(&self, _: &OrganisationId) -> Result<(), CoreError> {
            unreachable!()
        }
        async fn slug_exists(
            &self,
            _: &crate::organisation::value_objects::OrganisationSlug,
        ) -> Result<bool, CoreError> {
            unreachable!()
        }
        async fn count(&self) -> Result<usize, CoreError> {
            unreachable!()
        }
        async fn count_by_status(
            &self,
            _: crate::organisation::value_objects::OrganisationStatus,
        ) -> Result<usize, CoreError> {
            unreachable!()
        }
    }

    #[derive(Clone)]
    struct Roles(Vec<Role>);

    impl RoleRepository for Roles {
        async fn list_by_organisation(&self, _: OrganisationId) -> Result<Vec<Role>, CoreError> {
            Ok(self.0.clone())
        }
        async fn insert(&self, _: Role) -> Result<(), CoreError> {
            unreachable!()
        }
        async fn get_by_id(&self, _: RoleId) -> Result<Option<Role>, CoreError> {
            unreachable!()
        }
        async fn list_by_names(
            &self,
            _: OrganisationId,
            _: Vec<String>,
        ) -> Result<Vec<Role>, CoreError> {
            unreachable!("names decide nothing")
        }
        async fn update(&self, _: Role) -> Result<(), CoreError> {
            unreachable!()
        }
        async fn delete(&self, _: RoleId) -> Result<(), CoreError> {
            unreachable!()
        }
    }

    #[derive(Clone)]
    struct Users(Vec<User>);

    impl UserRepository for Users {
        async fn find_by_sub(&self, sub: &str) -> Result<Option<User>, CoreError> {
            Ok(self.0.iter().find(|user| user.sub == sub).cloned())
        }
        async fn find_by_email(&self, email: &str) -> Result<Option<User>, CoreError> {
            Ok(self
                .0
                .iter()
                .find(|user| user.email.eq_ignore_ascii_case(email))
                .cloned())
        }
        async fn upsert_by_email(&self, _: &User) -> Result<User, CoreError> {
            unreachable!("an invitation does not create the account")
        }
    }

    type Service = InvitationServiceImpl<Organisations, Store, Roles, Users, Allowed>;

    fn service(store: Store, roles: Vec<Role>, users: Vec<User>) -> Service {
        InvitationServiceImpl::new(
            Organisations(store.clone()),
            store,
            Roles(roles),
            Users(users),
            Allowed,
        )
    }

    fn email(raw: &str) -> InvitedEmail {
        InvitedEmail::parse(raw).expect("an address")
    }

    /// The rule the link's safety rests on. Without it the link is the
    /// credential, and a message forwarded by mistake is an account in
    /// somebody else's organisation.
    #[tokio::test]
    async fn somebody_it_was_not_written_to_cannot_walk_through_it() {
        let store = Store::default();
        let service = service(
            store.clone(),
            Vec::new(),
            vec![
                user(INVITER, "inviter@acme.test"),
                user(NEWCOMER, "somebody.else@acme.test"),
            ],
        );

        let (_, token) = service
            .invite(
                caller(INVITER),
                OrganisationId(ORGANISATION),
                email("colleague@acme.test"),
                Vec::new(),
            )
            .await
            .expect("invited");

        let error = service
            .accept_invitation(caller(NEWCOMER), token)
            .await
            .expect_err("not their invitation");

        assert!(matches!(
            error,
            CoreError::InvitationAddressedToSomebodyElse
        ));
        assert!(store.members().is_empty());
    }

    /// And the person it was written to gets in, holding what it promised.
    /// The two tests differ by one address.
    #[tokio::test]
    async fn the_person_it_was_written_to_gets_in_holding_what_it_promised() {
        let store = Store::default();
        let service = service(
            store.clone(),
            vec![role(MINE)],
            vec![
                user(INVITER, "inviter@acme.test"),
                user(NEWCOMER, "colleague@acme.test"),
            ],
        );

        let (_, token) = service
            .invite(
                caller(INVITER),
                OrganisationId(ORGANISATION),
                email("colleague@acme.test"),
                vec![RoleId(MINE)],
            )
            .await
            .expect("invited");

        service
            .accept_invitation(caller(NEWCOMER), token)
            .await
            .expect("in");

        assert_eq!(store.members(), vec![NEWCOMER]);
        assert_eq!(store.granted(), vec![RoleId(MINE)]);
    }

    /// One address however it was typed, on both sides.
    #[tokio::test]
    async fn the_address_matches_whatever_case_either_side_used() {
        let store = Store::default();
        let service = service(
            store.clone(),
            Vec::new(),
            vec![
                user(INVITER, "inviter@acme.test"),
                user(NEWCOMER, "Colleague@Acme.Test"),
            ],
        );

        let (_, token) = service
            .invite(
                caller(INVITER),
                OrganisationId(ORGANISATION),
                email("COLLEAGUE@acme.test"),
                Vec::new(),
            )
            .await
            .expect("invited");

        service
            .accept_invitation(caller(NEWCOMER), token)
            .await
            .expect("in");

        assert_eq!(store.members(), vec![NEWCOMER]);
    }

    #[tokio::test]
    async fn an_invitation_cannot_be_walked_through_twice() {
        let store = Store::default();
        let service = service(
            store.clone(),
            Vec::new(),
            vec![
                user(INVITER, "inviter@acme.test"),
                user(NEWCOMER, "colleague@acme.test"),
            ],
        );

        let (_, token) = service
            .invite(
                caller(INVITER),
                OrganisationId(ORGANISATION),
                email("colleague@acme.test"),
                Vec::new(),
            )
            .await
            .expect("invited");
        let again = InvitationToken::parse(token.expose()).expect("the same secret");

        service
            .accept_invitation(caller(NEWCOMER), token)
            .await
            .expect("in");

        let error = service
            .accept_invitation(caller(NEWCOMER), again)
            .await
            .expect_err("already used");

        assert!(matches!(error, CoreError::InvitationAlreadyAccepted));
    }

    /// Revoking cuts the link. It does not put anybody out: that is a
    /// different act and it needs a different right.
    #[tokio::test]
    async fn revoking_after_acceptance_leaves_the_membership_alone() {
        let store = Store::default();
        let service = service(
            store.clone(),
            Vec::new(),
            vec![
                user(INVITER, "inviter@acme.test"),
                user(NEWCOMER, "colleague@acme.test"),
            ],
        );

        let (invitation, token) = service
            .invite(
                caller(INVITER),
                OrganisationId(ORGANISATION),
                email("colleague@acme.test"),
                Vec::new(),
            )
            .await
            .expect("invited");

        service
            .accept_invitation(caller(NEWCOMER), token)
            .await
            .expect("in");

        service
            .revoke_invitation(caller(INVITER), OrganisationId(ORGANISATION), invitation.id)
            .await
            .expect("revoked");

        assert_eq!(store.members(), vec![NEWCOMER]);
    }

    #[tokio::test]
    async fn a_revoked_link_stops_working() {
        let store = Store::default();
        let service = service(
            store.clone(),
            Vec::new(),
            vec![
                user(INVITER, "inviter@acme.test"),
                user(NEWCOMER, "colleague@acme.test"),
            ],
        );

        let (invitation, token) = service
            .invite(
                caller(INVITER),
                OrganisationId(ORGANISATION),
                email("colleague@acme.test"),
                Vec::new(),
            )
            .await
            .expect("invited");

        service
            .revoke_invitation(caller(INVITER), OrganisationId(ORGANISATION), invitation.id)
            .await
            .expect("revoked");

        let error = service
            .accept_invitation(caller(NEWCOMER), token)
            .await
            .expect_err("cut off");

        assert!(matches!(error, CoreError::InvitationRevoked));
        assert!(store.members().is_empty());
    }

    /// An invitation promising a role from somewhere else would apply it on
    /// acceptance. That is the cross-organisation grant by another route.
    #[tokio::test]
    async fn a_role_from_another_organisation_cannot_be_promised() {
        let store = Store::default();
        let service = service(
            store.clone(),
            vec![role(MINE)],
            vec![user(INVITER, "inviter@acme.test")],
        );

        let error = service
            .invite(
                caller(INVITER),
                OrganisationId(ORGANISATION),
                email("colleague@acme.test"),
                vec![RoleId(SOMEBODY_ELSES)],
            )
            .await
            // `err()` rather than `expect_err`: the Ok side carries the token,
            // and the token has no Debug precisely so it cannot be printed.
            .err()
            .expect("not ours");

        assert!(matches!(error, CoreError::RoleNotInOrganisation { .. }));
    }

    /// Somebody already in does not need a way in. Issuing one anyway leaves
    /// a live link that does nothing and looks outstanding to whoever reads
    /// the list.
    #[tokio::test]
    async fn somebody_already_in_is_not_invited_again() {
        let store = Store::default();
        store.members.lock().expect("not poisoned").push(NEWCOMER);

        let service = service(
            store.clone(),
            Vec::new(),
            vec![
                user(INVITER, "inviter@acme.test"),
                user(NEWCOMER, "colleague@acme.test"),
            ],
        );

        let error = service
            .invite(
                caller(INVITER),
                OrganisationId(ORGANISATION),
                email("colleague@acme.test"),
                Vec::new(),
            )
            .await
            .err()
            .expect("already in");

        assert!(matches!(error, CoreError::AlreadyAMember { .. }));
    }

    /// Somebody with no account yet is exactly who invitations exist for.
    /// Issuing must not need them to have signed in.
    #[tokio::test]
    async fn somebody_who_has_never_signed_in_can_be_invited() {
        let store = Store::default();
        let service = service(
            store.clone(),
            Vec::new(),
            vec![user(INVITER, "inviter@acme.test")],
        );

        let (invitation, _) = service
            .invite(
                caller(INVITER),
                OrganisationId(ORGANISATION),
                email("nobody@acme.test"),
                Vec::new(),
            )
            .await
            .expect("invited");

        assert_eq!(invitation.invited_by, Some(UserId(INVITER)));
        assert_eq!(invitation.state(Utc::now()), InvitationState::Pending);
    }

    #[tokio::test]
    async fn an_expired_link_says_it_expired() {
        let store = Store::default();
        let token = InvitationToken::generate();
        let mut invitation = Invitation::issue(
            OrganisationId(ORGANISATION),
            email("colleague@acme.test"),
            Vec::new(),
            None,
            Utc::now(),
        )
        .0;
        invitation.expires_at = Utc::now() - Duration::hours(1);
        store.put(invitation, token.hash());

        let service = service(
            store.clone(),
            Vec::new(),
            vec![user(NEWCOMER, "colleague@acme.test")],
        );

        let error = service
            .accept_invitation(caller(NEWCOMER), token)
            .await
            .expect_err("expired");

        assert!(matches!(error, CoreError::InvitationExpired { .. }));
        assert!(store.members().is_empty());
    }

    #[tokio::test]
    async fn a_secret_nobody_issued_finds_nothing() {
        let store = Store::default();
        let service = service(
            store.clone(),
            Vec::new(),
            vec![user(NEWCOMER, "colleague@acme.test")],
        );

        let error = service
            .accept_invitation(caller(NEWCOMER), InvitationToken::generate())
            .await
            .expect_err("no such link");

        assert!(matches!(error, CoreError::InvitationNotFound));
    }
}
