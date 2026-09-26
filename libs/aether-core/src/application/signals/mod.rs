//! Signal operations for probes and the API.
//!
//! Writing and closing signals is not something a caller asks for -- it follows
//! from a fleet condition like a stale data plane. The write/close methods are
//! called by background probes with no Identity.
//!
//! Listing signals is a platform endpoint: requires identity-gating and the
//! ViewEstate right.

use aether_auth::Identity;
use aether_domain::signals::{
    Signal, SignalKind, SignalSubject,
    ports::{SignalListPage, SignalRepository},
};
use aether_macros::transactional;
use chrono::{DateTime, Utc};

use crate::{AetherService, CoreError, policy::PlatformRightsPolicy};

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

    /// List open signals, newest first, optionally filtered by kind and subject.
    ///
    /// Requires the caller to hold ViewEstate.
    #[transactional(signal)]
    pub async fn list_open_signals(
        &self,
        identity: Identity,
        kind_filter: Option<SignalKind>,
        subject_filter: Option<SignalSubject>,
        limit: usize,
        cursor: Option<String>,
    ) -> Result<SignalListPage, CoreError> {
        let service = aether_domain::signals::service::SignalServiceImpl::new(
            signal_repository,
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
        );

        service
            .list_open_signals(identity, kind_filter, subject_filter, limit, cursor)
            .await
    }
}
