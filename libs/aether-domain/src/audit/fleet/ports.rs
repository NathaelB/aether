use std::future::Future;

use aether_auth::Identity;

use crate::{
    CoreError,
    audit::{
        AuditCursor,
        fleet::{FleetAuditBatch, FleetAuditEntry, commands::ListFleetAuditEntriesCommand},
    },
};

/// Append only, like [`crate::audit::ports::AuditRepository`]: no update, no
/// delete, not even one nobody calls yet.
#[cfg_attr(test, mockall::automock)]
pub trait FleetAuditRepository: Send + Sync {
    fn append(&self, entry: FleetAuditEntry) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// The whole installation's trail, newest first, keyset-paginated on
    /// `(recorded_at, id)`.
    ///
    /// Takes no scope, and that is the point: there is no organisation to
    /// pass, so there is no query here a customer's request could be made to
    /// satisfy.
    fn list(
        &self,
        cursor: Option<AuditCursor>,
        limit: usize,
    ) -> impl Future<Output = Result<FleetAuditBatch, CoreError>> + Send;
}

/// Reading only.
///
/// Recording has no method here on purpose. A fleet action is written by the
/// service that performed it, through [`FleetAuditRepository`] directly and
/// inside that service's own transaction -- the same shape
/// [`crate::platform::plan::TenantPlanServiceImpl`] uses for the organisation
/// trail. A `record` on this trait would be a second entry point, reachable
/// without having performed anything, and the permission check that makes an
/// entry true lives in the act rather than in the writing of it.
#[cfg_attr(test, mockall::automock)]
pub trait FleetAuditService: Send + Sync {
    fn list_fleet_entries(
        &self,
        identity: Identity,
        command: ListFleetAuditEntriesCommand,
    ) -> impl Future<Output = Result<FleetAuditBatch, CoreError>> + Send;
}
