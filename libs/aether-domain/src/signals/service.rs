//! Signal service.
//!
//! In this workstream (V0), the service is a placeholder. Probes will write to
//! the repository directly in their own transactions. Reading comes later in V2.

use chrono::{DateTime, Utc};

use crate::CoreError;

use super::{Signal, ports::SignalRepository};

pub struct SignalServiceImpl<R>
where
    R: SignalRepository,
{
    signals: R,
}

impl<R> SignalServiceImpl<R>
where
    R: SignalRepository,
{
    pub fn new(signals: R) -> Self {
        Self { signals }
    }

    /// Write or update a signal.
    ///
    /// If a signal with the same dedup_key is already open, that signal's
    /// last_seen_at and message are updated instead of inserting a new row.
    pub async fn write_signal(&self, signal: Signal) -> Result<(), CoreError> {
        self.signals.write(signal).await
    }

    /// Close the open signal with the given dedup_key.
    ///
    /// Sets closed_at on the signal matching this dedup_key if it is currently
    /// open (closed_at IS NULL). If no open signal exists for this dedup_key,
    /// this is a no-op and returns Ok(()) (idempotent close).
    pub async fn close_signal(&self, dedup_key: &str, at: DateTime<Utc>) -> Result<(), CoreError> {
        self.signals.close(dedup_key, at).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    use crate::{
        dataplane::value_objects::DataPlaneId,
        signals::{SignalId, SignalKind, SignalSubject},
    };

    #[tokio::test]
    async fn the_service_writes_a_signal() {
        let mut signals = super::super::ports::MockSignalRepository::new();
        signals
            .expect_write()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));

        let service = SignalServiceImpl::new(signals);
        let signal = Signal::open(
            SignalId(Uuid::new_v4()),
            SignalKind::DataplaneHeartbeatStale,
            SignalSubject::Dataplane {
                id: DataPlaneId(Uuid::new_v4()),
            },
            "key-1".to_string(),
            "Dataplane is stale".to_string(),
            Utc::now(),
        );

        assert!(service.write_signal(signal).await.is_ok());
    }

    #[tokio::test]
    async fn the_service_closes_a_signal() {
        let mut signals = super::super::ports::MockSignalRepository::new();
        signals
            .expect_close()
            .times(1)
            .withf(|key, _| key == "key-1")
            .returning(|_, _| Box::pin(async { Ok(()) }));

        let service = SignalServiceImpl::new(signals);

        assert!(service.close_signal("key-1", Utc::now()).await.is_ok());
    }
}
