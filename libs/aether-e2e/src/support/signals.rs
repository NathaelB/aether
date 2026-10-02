use std::sync::{Arc, Mutex};

use aether_domain::CoreError;
use aether_domain::dataplane::entities::DataPlane;
use aether_domain::dataplane::value_objects::DataPlaneLiveness;
use aether_domain::platform::ports::PlatformPolicy;
use aether_domain::signals::ports::{SignalListPage, SignalRepository};
use aether_domain::signals::service::SignalServiceImpl;
use aether_domain::signals::{Signal, SignalId, SignalKind, SignalSubject};
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct InMemorySignals(Arc<Mutex<Vec<Signal>>>);

impl InMemorySignals {
    pub fn rows(&self) -> Vec<Signal> {
        self.0.lock().expect("not poisoned").clone()
    }

    pub fn open_rows(&self) -> Vec<Signal> {
        self.rows()
            .into_iter()
            .filter(|signal| signal.closed_at.is_none())
            .collect()
    }

    pub fn closed_rows(&self) -> Vec<Signal> {
        self.rows()
            .into_iter()
            .filter(|signal| signal.closed_at.is_some())
            .collect()
    }
}

impl SignalRepository for InMemorySignals {
    async fn write(&self, signal: Signal) -> Result<(), CoreError> {
        let mut rows = self.0.lock().expect("not poisoned");
        let open = rows
            .iter_mut()
            .find(|row| row.closed_at.is_none() && row.dedup_key == signal.dedup_key);
        match open {
            Some(row) => {
                row.last_seen_at = signal.last_seen_at;
                row.message = signal.message;
            }
            None => rows.push(signal),
        }
        Ok(())
    }

    async fn close(&self, dedup_key: &str, at: DateTime<Utc>) -> Result<(), CoreError> {
        let mut rows = self.0.lock().expect("not poisoned");
        if let Some(row) = rows
            .iter_mut()
            .find(|row| row.closed_at.is_none() && row.dedup_key == dedup_key)
        {
            row.closed_at = Some(at);
        }
        Ok(())
    }

    async fn list_open(
        &self,
        kind_filter: Option<SignalKind>,
        subject_filter: Option<SignalSubject>,
        limit: usize,
        _: Option<String>,
    ) -> Result<SignalListPage, CoreError> {
        let mut signals: Vec<Signal> = self
            .open_rows()
            .into_iter()
            .filter(|signal| kind_filter.is_none_or(|kind| signal.kind == kind))
            .filter(|signal| {
                subject_filter
                    .as_ref()
                    .is_none_or(|subject| &signal.subject == subject)
            })
            .collect();
        signals.sort_by_key(|signal| std::cmp::Reverse(signal.opened_at));
        signals.truncate(limit);
        Ok(SignalListPage {
            signals,
            next_cursor: None,
        })
    }
}

pub fn heartbeat_stale_key(plane: &DataPlane) -> String {
    format!("dataplane-heartbeat-stale-{}", plane.id.0)
}

pub fn open_signal(
    kind: SignalKind,
    subject: SignalSubject,
    dedup_key: String,
    message: String,
    at: DateTime<Utc>,
) -> Signal {
    Signal::open(
        SignalId(Uuid::new_v4()),
        kind,
        subject,
        dedup_key,
        message,
        at,
    )
}

pub async fn settle_heartbeat_stale_signal<R, P>(
    signals: &SignalServiceImpl<R, P>,
    plane: &DataPlane,
    now: DateTime<Utc>,
    window: Duration,
) -> Result<(), CoreError>
where
    R: SignalRepository,
    P: PlatformPolicy,
{
    match plane.liveness(now, window) {
        DataPlaneLiveness::Unreachable => {
            let silent_for = plane
                .last_seen_at
                .map(|seen| now - seen)
                .unwrap_or_default();
            let message = format!(
                "Data plane {} has not reported a heartbeat for {} seconds",
                plane.id.0,
                silent_for.num_seconds()
            );
            let signal = open_signal(
                SignalKind::DataplaneHeartbeatStale,
                SignalSubject::Dataplane { id: plane.id },
                heartbeat_stale_key(plane),
                message,
                now,
            );
            signals.write_signal(signal).await
        }
        DataPlaneLiveness::Reachable | DataPlaneLiveness::NeverSeen => {
            signals.close_signal(&heartbeat_stale_key(plane), now).await
        }
    }
}
