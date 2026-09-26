//! Signal write and close operations for system probes.
//!
//! Writing and closing signals is not something a caller asks for -- it follows
//! from a fleet condition like a stale data plane, the same way purging deleted
//! deployments follows from a retention window rather than a request. The methods
//! below serve that, called by background probes. None takes an [`aether_auth::Identity`]
//! for the same reason [`crate::AetherService::purge_deleted_deployments`] does not:
//! nobody is the caller, the installation's own upkeep is.

use aether_domain::signals::{Signal, ports::SignalRepository};
use aether_macros::transactional;
use chrono::{DateTime, Utc};

use crate::{AetherService, CoreError};

impl AetherService {
    /// Write or update a signal.
    ///
    /// If a signal with the same dedup_key is already open (closed_at IS NULL),
    /// that signal's last_seen_at and message are updated instead of inserting
    /// a new row. This invariant is enforced at the schema level via a partial
    /// unique index on dedup_key WHERE closed_at IS NULL.
    #[transactional(signal)]
    pub async fn write_signal(&self, signal: Signal) -> Result<(), CoreError> {
        signal_repository.write(signal).await
    }

    /// Close the open signal with the given dedup_key.
    ///
    /// Sets closed_at on the signal matching this dedup_key if it is currently
    /// open (closed_at IS NULL). If no open signal exists for this dedup_key,
    /// this is a no-op and returns Ok(()) (idempotent close).
    #[transactional(signal)]
    pub async fn close_signal(&self, dedup_key: &str, at: DateTime<Utc>) -> Result<(), CoreError> {
        signal_repository.close(dedup_key, at).await
    }
}
