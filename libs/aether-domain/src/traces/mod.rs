//! Reading an instance's OpenTelemetry traces.
//!
//! Mirrors `crate::logs`: nothing here is written by the control plane
//! either, a search reaches straight into the organisation's own trace
//! index (`traces-{organisation_id}`, shipped by Herald's OTLP receiver),
//! and reading it leaves the same kind of audit entry a log search does.
//! The window, the search cap and the histogram bucketing are shared
//! outright with `crate::logs` rather than duplicated, since both search
//! the same Quickwit installation under the same 30-day retention.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    deployments::DeploymentId,
    logs::{LogSearchBucket, LogSearchWindow},
};

pub mod commands;
pub mod ports;
pub mod service;

/// How many spans a single search answers with, regardless of how many
/// matched -- the same reasoning as `crate::logs::MAX_SEARCH_HITS`.
pub const MAX_SEARCH_HITS: usize = 200;

/// How many distinct values a single facet reports -- the same reasoning as
/// `crate::logs::MAX_FACET_TERMS`.
pub const MAX_FACET_TERMS: usize = 20;

/// The translated request a trace index adapter receives -- everything
/// [`commands::SearchTracesCommand`] carries except the tenant, which
/// travels as its own argument on [`ports::TraceSearchIndex`] for the same
/// reason `crate::logs::LogSearchFilter` keeps it out: the one thing that
/// picks which index is even reachable must never be something a filter
/// value could carry instead.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceSearchFilter {
    pub deployment_id: Option<DeploymentId>,
    pub window: LogSearchWindow,
    pub service_name: Option<String>,
    pub status_code: Option<String>,
    pub text: Option<String>,
}

/// One span the index matched.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct SpanHit {
    pub trace_id: String,
    pub span_id: String,
    /// Empty for a root span -- see `herald_core::domain::trace_index`'s own
    /// doc comment on the same field.
    pub parent_span_id: String,
    pub deployment_id: DeploymentId,
    pub service_name: String,
    pub name: String,
    pub kind: String,
    pub start_timestamp: DateTime<Utc>,
    pub duration_nanos: u64,
    pub status_code: String,
    pub status_message: String,
}

/// One distinct value a facet found, and how many matching spans carried it.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct TraceFacetBucket {
    pub value: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct TraceFacets {
    pub service_name: Vec<TraceFacetBucket>,
    pub status_code: Vec<TraceFacetBucket>,
}

/// What a search answers with: up to [`MAX_SEARCH_HITS`] spans, how many
/// actually matched, facets over that same matching set, and a
/// time-bucketed histogram at `crate::logs::histogram_interval`'s width for
/// the requested window.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct TraceSearchResult {
    pub hits: Vec<SpanHit>,
    pub total_hits: u64,
    pub facets: TraceFacets,
    pub buckets: Vec<LogSearchBucket>,
}

/// Every span sharing one trace id, ordered by `start_timestamp` -- what a
/// waterfall view is built from. Not capped the way [`MAX_SEARCH_HITS`]
/// caps a search: a trace with an unusual number of spans is still one
/// trace, and truncating it would draw a waterfall missing the very spans
/// an operator opened it to see.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct TraceDetail {
    pub trace_id: String,
    pub spans: Vec<SpanHit>,
}
