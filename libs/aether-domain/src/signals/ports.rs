use std::future::Future;

use chrono::{DateTime, Utc};

use crate::CoreError;

use super::{Signal, SignalKind, SignalSubject};

/// Write, update, and close signals.
#[cfg_attr(test, mockall::automock)]
pub trait SignalRepository: Send + Sync {
    /// Write or update a signal.
    ///
    /// If a signal with the same dedup_key is already open (closed_at IS NULL),
    /// that signal's last_seen_at and message are updated instead of inserting
    /// a new row. This invariant is enforced at the schema level via a partial
    /// unique index on dedup_key WHERE closed_at IS NULL.
    fn write(&self, signal: Signal) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// Close the open signal with the given dedup_key.
    ///
    /// Sets closed_at on the signal matching this dedup_key if it is currently
    /// open (closed_at IS NULL). If no open signal exists for this dedup_key,
    /// this is a no-op and returns Ok(()) (idempotent close).
    fn close(
        &self,
        dedup_key: &str,
        at: DateTime<Utc>,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// List open signals, optionally filtered by kind and subject, newest first.
    fn list_open(
        &self,
        kind_filter: Option<SignalKind>,
        subject_filter: Option<SignalSubject>,
        limit: usize,
        cursor: Option<String>,
    ) -> impl Future<Output = Result<SignalListPage, CoreError>> + Send;
}

/// One page of open signals, with a cursor for the next page.
#[derive(Debug, Clone, PartialEq)]
pub struct SignalListPage {
    pub signals: Vec<Signal>,
    pub next_cursor: Option<String>,
}
