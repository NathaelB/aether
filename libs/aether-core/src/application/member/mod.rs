use aether_auth::Identity;
use aether_domain::{
    CoreError,
    organisation::{
        OrganisationId, member::Member, member_service::MemberServiceImpl, ports::MemberService,
    },
    role::RoleId,
    user::UserId,
};
use aether_macros::transactional;

use crate::{AetherService, infrastructure::role::permissions_in, policy::AetherPolicy};

impl MemberService for AetherService {
    #[transactional(organisation, role)]
    async fn list_members(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Member>, CoreError> {
        MemberServiceImpl::new(
            organisation_repository,
            role_repository,
            AetherPolicy::new(permissions_in(&tx)),
        )
        .list_members(identity, organisation_id)
        .await
    }

    #[transactional(organisation, role)]
    async fn member(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        user_id: UserId,
    ) -> Result<Member, CoreError> {
        MemberServiceImpl::new(
            organisation_repository,
            role_repository,
            AetherPolicy::new(permissions_in(&tx)),
        )
        .member(identity, organisation_id, user_id)
        .await
    }

    #[transactional(organisation, role)]
    async fn set_member_roles(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        user_id: UserId,
        roles: Vec<RoleId>,
    ) -> Result<Member, CoreError> {
        MemberServiceImpl::new(
            organisation_repository,
            role_repository,
            AetherPolicy::new(permissions_in(&tx)),
        )
        .set_member_roles(identity, organisation_id, user_id, roles)
        .await
    }

    #[transactional(organisation, role)]
    async fn remove_member(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        user_id: UserId,
    ) -> Result<(), CoreError> {
        MemberServiceImpl::new(
            organisation_repository,
            role_repository,
            AetherPolicy::new(permissions_in(&tx)),
        )
        .remove_member(identity, organisation_id, user_id)
        .await
    }
}
