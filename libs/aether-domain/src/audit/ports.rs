use std::future::Future;

use aether_auth::Identity;

use crate::{
    CoreError,
    audit::{
        AuditBatch, AuditCursor, AuditEntry,
        commands::{ListAuditEntriesCommand, RecordAuditEntryCommand},
    },
    organisation::OrganisationId,
};

/// Append only. No update, no delete, not even one nobody calls yet -- a
/// method that could edit a written entry is the one somebody eventually
/// does.
#[cfg_attr(test, mockall::automock)]
pub trait AuditRepository: Send + Sync {
    fn append(&self, entry: AuditEntry) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// One organisation's trail, newest first, keyset-paginated on
    /// `(recorded_at, id)`.
    ///
    /// Newest first rather than oldest first: a page is bounded by "older
    /// than the last entry this reader saw", and a newly recorded entry sorts
    /// ahead of that boundary. It can never shift a page a reader has already
    /// fetched, the way an `OFFSET` would.
    fn list_for_organisation(
        &self,
        organisation_id: OrganisationId,
        cursor: Option<AuditCursor>,
        limit: usize,
    ) -> impl Future<Output = Result<AuditBatch, CoreError>> + Send;
}

/// Recording is un-gated by design: the caller is another domain service that
/// already decided, under its own permission check, that the write it is
/// describing was allowed. Re-deriving that decision here would be a second,
/// divergent copy of a check that already happened once.
///
/// Reading is scoped to one organisation and gated on `identity`, because the
/// trail is read directly by whoever asks, with no other check upstream of
/// it.
#[cfg_attr(test, mockall::automock)]
pub trait AuditService: Send + Sync {
    fn record(
        &self,
        command: RecordAuditEntryCommand,
    ) -> impl Future<Output = Result<AuditEntry, CoreError>> + Send;

    fn list_entries(
        &self,
        identity: Identity,
        command: ListAuditEntriesCommand,
    ) -> impl Future<Output = Result<AuditBatch, CoreError>> + Send;
}
