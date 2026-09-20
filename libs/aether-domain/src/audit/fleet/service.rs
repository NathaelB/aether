use aether_auth::Identity;

use crate::{
    CoreError,
    audit::fleet::{
        FleetActor, FleetAuditBatch,
        commands::ListFleetAuditEntriesCommand,
        ports::{FleetAuditRepository, FleetAuditService},
    },
    platform::{PlatformRight, ports::PlatformPolicy},
};

/// Who the fleet trail names for something asked for through the API.
///
/// One function, because this is a decision rather than a lookup, and the
/// counterpart to [`crate::audit::service::audit_actor`]. It differs from that
/// one in the way the whole trail does: it resolves nothing and so cannot
/// fail. A subject is what the identity provider issued and what
/// `platform_operators` keys rights on, which makes it both available here and
/// the name the installation actually decided with.
pub fn fleet_actor(identity: &Identity) -> FleetActor {
    match identity {
        Identity::Client(client) => FleetActor::Api {
            client_id: client.client_id.clone(),
        },
        Identity::User(_) => FleetActor::Operator {
            subject: identity.id().to_string(),
        },
    }
}

pub struct FleetAuditServiceImpl<R, P>
where
    R: FleetAuditRepository,
    P: PlatformPolicy,
{
    entries: R,
    policy: P,
}

impl<R, P> FleetAuditServiceImpl<R, P>
where
    R: FleetAuditRepository,
    P: PlatformPolicy,
{
    pub fn new(entries: R, policy: P) -> Self {
        Self { entries, policy }
    }
}

impl<R, P> FleetAuditService for FleetAuditServiceImpl<R, P>
where
    R: FleetAuditRepository,
    P: PlatformPolicy,
{
    /// Gated on `view_estate` rather than on `operate_fleet`.
    ///
    /// Reading what was done to the fleet is part of seeing it, the same way
    /// [`crate::platform::service::PlatformServiceImpl::list_operators`] is:
    /// somebody trusted to see every tenant learning who drained a cluster is
    /// not the leak. Gating it on `operate_fleet` would mean only the people
    /// who can take a cluster offline may read who did -- which is the one
    /// arrangement an audit trail exists to avoid.
    ///
    /// No narrower gate on the operator entries either. Splitting the trail by
    /// right would leave a reader with a list full of holes and no way to tell
    /// a quiet week from a filtered one.
    async fn list_fleet_entries(
        &self,
        identity: Identity,
        command: ListFleetAuditEntriesCommand,
    ) -> Result<FleetAuditBatch, CoreError> {
        self.policy
            .require(identity, PlatformRight::ViewEstate)
            .await?;

        self.entries.list(command.cursor, command.limit).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        audit::{AuditCursor, fleet::ports::MockFleetAuditRepository},
        platform::fixtures::Granting,
    };
    use aether_auth::User;

    fn identity() -> Identity {
        Identity::User(User {
            id: "operator-1".to_string(),
            username: "operator".to_string(),
            email: None,
            name: None,
            roles: vec![],
        })
    }

    fn empty_batch() -> FleetAuditBatch {
        FleetAuditBatch {
            entries: vec![],
            next_cursor: None,
        }
    }

    #[tokio::test]
    async fn an_operator_who_may_see_the_estate_reads_the_trail() {
        let mut entries = MockFleetAuditRepository::new();
        entries
            .expect_list()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(empty_batch()) }));

        let service =
            FleetAuditServiceImpl::new(entries, Granting::only(PlatformRight::ViewEstate));

        assert!(
            service
                .list_fleet_entries(identity(), ListFleetAuditEntriesCommand::new(50))
                .await
                .is_ok()
        );
    }

    /// The repository is never reached, not merely ignored: a refusal that
    /// read the rows first would have already loaded fleet actions into a
    /// process serving somebody who may not see them.
    #[tokio::test]
    async fn without_the_right_the_trail_is_not_read_at_all() {
        let mut entries = MockFleetAuditRepository::new();
        entries.expect_list().never();

        let service = FleetAuditServiceImpl::new(entries, Granting::nothing());
        let refused = service
            .list_fleet_entries(identity(), ListFleetAuditEntriesCommand::new(50))
            .await
            .expect_err("a nobody read the fleet trail");

        let CoreError::MissingPlatformRight { right } = refused else {
            panic!("the refusal did not name a right: {refused}");
        };
        assert_eq!(right, "view_estate");
    }

    /// Holding `operate_fleet` is not, on its own, permission to read what was
    /// done to the fleet -- and holding `view_estate` is. Stated as a test
    /// because it is the one gate somebody would reasonably guess the other
    /// way round.
    #[tokio::test]
    async fn changing_the_fleet_is_not_permission_to_read_its_trail() {
        let mut entries = MockFleetAuditRepository::new();
        entries.expect_list().never();

        let service =
            FleetAuditServiceImpl::new(entries, Granting::only(PlatformRight::OperateFleet));

        assert!(
            service
                .list_fleet_entries(identity(), ListFleetAuditEntriesCommand::new(50))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn the_cursor_and_limit_travel_to_the_repository_unchanged() {
        let mut entries = MockFleetAuditRepository::new();
        entries
            .expect_list()
            .times(1)
            .withf(|cursor, limit| *cursor == Some(AuditCursor::new("cursor-1")) && *limit == 25)
            .returning(|_, _| Box::pin(async { Ok(empty_batch()) }));

        let service =
            FleetAuditServiceImpl::new(entries, Granting::only(PlatformRight::ViewEstate));
        let command =
            ListFleetAuditEntriesCommand::new(25).with_cursor(AuditCursor::new("cursor-1"));

        assert!(
            service
                .list_fleet_entries(identity(), command)
                .await
                .is_ok()
        );
    }
}
