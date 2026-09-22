use std::future::Future;

use crate::{
    CoreError,
    organisation::OrganisationId,
    traces::{TraceDetail, TraceSearchFilter, TraceSearchResult},
};

/// Where a trace search actually runs.
///
/// Mirrors `crate::logs::ports::LogSearchIndex`: its own port, separate
/// from `herald-core::infrastructure::traces::quickwit`'s ingest sink, for
/// the same reason that one is separate from the logs ingest sink --
/// shipping and searching are different capabilities held by different
/// processes.
///
/// `organisation_id` is its own argument rather than a field on
/// [`TraceSearchFilter`], for the same tenant-isolation reasoning
/// `LogSearchIndex` documents on its own `organisation_id` argument.
#[cfg_attr(test, mockall::automock)]
pub trait TraceSearchIndex: Send + Sync {
    fn search(
        &self,
        organisation_id: OrganisationId,
        filter: TraceSearchFilter,
    ) -> impl Future<Output = Result<TraceSearchResult, CoreError>> + Send;

    /// Every span sharing one trace id, for the waterfall view -- not scoped
    /// by [`TraceSearchFilter`], since a reader who already has a trace id
    /// (from a search hit) is asking for that trace whole, not for it
    /// narrowed by the search that found it.
    fn trace(
        &self,
        organisation_id: OrganisationId,
        trace_id: String,
    ) -> impl Future<Output = Result<TraceDetail, CoreError>> + Send;
}
