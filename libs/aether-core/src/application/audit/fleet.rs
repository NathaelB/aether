use aether_auth::Identity;
use aether_domain::{
    CoreError,
    audit::fleet::{
        FleetAuditBatch, commands::ListFleetAuditEntriesCommand, ports::FleetAuditService,
        service::FleetAuditServiceImpl,
    },
};
use aether_macros::transactional;

use crate::{AetherService, policy::PlatformRightsPolicy};

/// Reading only.
///
/// There is no `record` here to match [`FleetAuditService`]: a fleet action is
/// written by the service that performed it, inside that service's own
/// transaction, so the entry and the act it describes commit together or not
/// at all. See [`FleetAuditService`] for why that is a rule rather than a
/// convenience.
impl FleetAuditService for AetherService {
    #[transactional(fleet_audit, platform_operator)]
    async fn list_fleet_entries(
        &self,
        identity: Identity,
        command: ListFleetAuditEntriesCommand,
    ) -> Result<FleetAuditBatch, CoreError> {
        FleetAuditServiceImpl::new(
            fleet_audit_repository,
            PlatformRightsPolicy::new(platform_operator_repository),
        )
        .list_fleet_entries(identity, command)
        .await
    }
}
