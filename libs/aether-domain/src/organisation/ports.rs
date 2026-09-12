use std::future::Future;

use aether_auth::Identity;

use crate::{
    CoreError,
    organisation::member::Member,
    organisation::{
        Organisation, OrganisationId,
        commands::{CreateOrganisationCommand, CreateOrganisationData, UpdateOrganisationCommand},
        invitation::{
            Invitation, InvitationId, InvitationToken, InvitationTokenHash, InvitedEmail,
        },
        value_objects::{OrganisationSlug, OrganisationStatus},
    },
    role::RoleId,
    user::UserId,
};

/// Service trait for organisation business logic
pub trait OrganisationService: Send + Sync {
    /// Creates a new organisation
    fn create_organisation(
        &self,
        command: CreateOrganisationCommand,
    ) -> impl Future<Output = Result<Organisation, CoreError>> + Send;

    /// Updates an existing organisation
    fn update_organisation(
        &self,
        id: OrganisationId,
        command: UpdateOrganisationCommand,
    ) -> impl Future<Output = Result<Organisation, CoreError>> + Send;

    /// Deletes an organisation
    fn delete_organisation(
        &self,
        id: OrganisationId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn get_organisations(
        &self,
        status: Option<OrganisationStatus>,
        limit: usize,
        offset: usize,
    ) -> impl Future<Output = Result<Vec<Organisation>, CoreError>> + Send;

    fn get_organisations_by_member(
        &self,
        identity: Identity,
    ) -> impl Future<Output = Result<Vec<Organisation>, CoreError>> + Send;
}

/// Repository trait for organisation persistence
///
/// This trait defines the contract for persisting and retrieving organisations
/// Following the hexagonal architecture pattern (ports & adapters)
///
/// The repository is responsible for:
/// - Generating IDs (UUID v4)
/// - Managing timestamps (created_at, updated_at, deleted_at)
/// - Persisting to the database
#[cfg_attr(test, mockall::automock)]
pub trait OrganisationRepository: Send + Sync {
    /// Creates a new organisation
    /// The repository will generate the ID and timestamps
    fn create(
        &self,
        data: CreateOrganisationData,
    ) -> impl Future<Output = Result<Organisation, CoreError>> + Send;

    fn insert_member(
        &self,
        organisation_id: &OrganisationId,
        user_id: &UserId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// Finds an organisation by its ID
    fn find_by_id(
        &self,
        id: &OrganisationId,
    ) -> impl Future<Output = Result<Option<Organisation>, CoreError>> + Send;

    /// Finds an organisation by its slug
    fn find_by_slug(
        &self,
        slug: &OrganisationSlug,
    ) -> impl Future<Output = Result<Option<Organisation>, CoreError>> + Send;

    /// Finds all organisations owned by a user
    fn find_by_owner(
        &self,
        owner_id: &UserId,
    ) -> impl Future<Output = Result<Vec<Organisation>, CoreError>> + Send;

    /// Finds all organisations where a user is a member
    /// This user's place in this organisation, with the roles they hold.
    ///
    /// `None` is not a member. `Some` with no roles is a member who may do
    /// nothing yet, which is a different fact and has to stay one: the first
    /// is refused, the second is shown the organisation and waits for a role.
    ///
    /// One read rather than a membership check and then a role lookup. Every
    /// authorisation decision takes this path.
    fn find_member(
        &self,
        organisation_id: &OrganisationId,
        user_id: &UserId,
    ) -> impl Future<Output = Result<Option<Member>, CoreError>> + Send;

    fn list_members(
        &self,
        organisation_id: &OrganisationId,
    ) -> impl Future<Output = Result<Vec<Member>, CoreError>> + Send;

    /// Replaces the set of roles a member holds.
    ///
    /// Every role must already belong to the organisation; the caller checks
    /// that, because a repository silently dropping the ones that do not
    /// would report a grant that never happened.
    fn set_member_roles(
        &self,
        organisation_id: &OrganisationId,
        user_id: &UserId,
        roles: &[RoleId],
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn remove_member(
        &self,
        organisation_id: &OrganisationId,
        user_id: &UserId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn find_by_member(
        &self,
        member_id: &UserId,
    ) -> impl Future<Output = Result<Vec<Organisation>, CoreError>> + Send;

    /// Lists all organisations with optional filters
    fn list(
        &self,
        status: Option<OrganisationStatus>,
        limit: usize,
        offset: usize,
    ) -> impl Future<Output = Result<Vec<Organisation>, CoreError>> + Send;

    /// Updates an existing organisation
    fn update(
        &self,
        organisation: Organisation,
    ) -> impl Future<Output = Result<Organisation, CoreError>> + Send;

    /// Deletes an organisation (soft delete)
    fn delete(&self, id: &OrganisationId) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// Checks if a slug is already taken
    fn slug_exists(
        &self,
        slug: &OrganisationSlug,
    ) -> impl Future<Output = Result<bool, CoreError>> + Send;

    /// Counts total organisations
    fn count(&self) -> impl Future<Output = Result<usize, CoreError>> + Send;

    /// Counts organisations by status
    fn count_by_status(
        &self,
        status: OrganisationStatus,
    ) -> impl Future<Output = Result<usize, CoreError>> + Send;
}

/// Who may look at an organisation's members, and who may change them.
///
/// Three rights rather than one. Seeing who is in an organisation, changing
/// what somebody may do, and putting somebody out are different acts, and the
/// bits for them were reserved before anything used them.
pub trait MemberPolicy: Send + Sync {
    fn can_view_members(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn can_manage_members(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn can_remove_members(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;
}

pub trait InvitationPolicy: Send + Sync {
    fn can_view_invitations(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn can_invite_members(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;
}

pub trait InvitationService: Send + Sync {
    /// Writes an invitation and hands back the secret that opens it.
    ///
    /// The secret is returned rather than stored, and this is the only moment
    /// it exists outside somebody's inbox. Nothing later can show it again.
    fn invite(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        email: InvitedEmail,
        roles: Vec<RoleId>,
    ) -> impl Future<Output = Result<(Invitation, InvitationToken), CoreError>> + Send;

    fn list_invitations(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<Vec<Invitation>, CoreError>> + Send;

    fn revoke_invitation(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        invitation_id: InvitationId,
    ) -> impl Future<Output = Result<Invitation, CoreError>> + Send;

    /// Walks somebody through their invitation into the organisation.
    ///
    /// Not gated on any permission in that organisation -- the invitation is
    /// the authorisation, which is the whole point of one. It is gated on the
    /// caller being the person it was written to.
    fn accept_invitation(
        &self,
        identity: Identity,
        token: InvitationToken,
    ) -> impl Future<Output = Result<Member, CoreError>> + Send;
}

pub trait MemberService: Send + Sync {
    fn list_members(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<Vec<Member>, CoreError>> + Send;

    fn member(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        user_id: UserId,
    ) -> impl Future<Output = Result<Member, CoreError>> + Send;

    /// Replaces the roles a member holds, rather than adding or removing one.
    ///
    /// The same reasoning the allow list uses: two people editing through add
    /// and remove calls converge on a set neither of them wrote.
    fn set_member_roles(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        user_id: UserId,
        roles: Vec<RoleId>,
    ) -> impl Future<Output = Result<Member, CoreError>> + Send;

    fn remove_member(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        user_id: UserId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;
}

/// Invitations, kept apart from the organisation repository.
///
/// Six methods about a different thing was the organisation repository doing
/// three jobs, and every test double of it having to answer questions about
/// invitations it never asks.
pub trait InvitationRepository: Send + Sync {
    fn save_invitation(
        &self,
        invitation: &Invitation,
        token_hash: &InvitationTokenHash,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn list_invitations(
        &self,
        organisation_id: &OrganisationId,
    ) -> impl Future<Output = Result<Vec<Invitation>, CoreError>> + Send;

    /// Found by the hash of what was presented, never by the secret itself.
    fn find_invitation_by_hash(
        &self,
        token_hash: &InvitationTokenHash,
    ) -> impl Future<Output = Result<Option<Invitation>, CoreError>> + Send;

    fn find_invitation(
        &self,
        organisation_id: &OrganisationId,
        invitation_id: &InvitationId,
    ) -> impl Future<Output = Result<Option<Invitation>, CoreError>> + Send;

    fn mark_invitation_accepted(
        &self,
        invitation_id: &InvitationId,
        at: chrono::DateTime<chrono::Utc>,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn mark_invitation_revoked(
        &self,
        invitation_id: &InvitationId,
        at: chrono::DateTime<chrono::Utc>,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;
}
