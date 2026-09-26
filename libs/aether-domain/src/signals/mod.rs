//! Fleet-wide signals: outcomes watched by operational probes.
//!
//! A `Signal` is a statement about something that happened or is happening in
//! the fleet, like a silent data plane or an unreachable deployment. Signals are
//! explicitly NOT incidents: no severity, no ownership, no paging here. Probes
//! write them, and an API will later read them.
//!
//! A signal with a given `dedup_key` that is already open (closed_at IS NULL)
//! is updated instead of being duplicated: the last_seen_at field is refreshed,
//! and the message is replaced. This invariant is enforced at the schema level
//! via a partial unique index on dedup_key WHERE closed_at IS NULL.

use std::fmt;

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{CoreError, dataplane::value_objects::DataPlaneId, deployments::DeploymentId};

pub mod ports;
pub mod service;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, ToSchema)]
pub struct SignalId(pub Uuid);

/// The kind of problem a signal describes.
///
/// A closed enum where every variant is named in one place. Adding a variant
/// requires touching only this file: match arms throughout the codebase will
/// be exhaustive and force the addition to be handled everywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SignalKind {
    /// A data plane has stopped sending heartbeats.
    DataplaneHeartbeatStale,
    /// A deployment did not answer its own health check.
    DeploymentUnreachable,
    /// An expected backup was not found.
    BackupMissing,
    /// A backup operation failed.
    BackupFailed,
    /// A scheduled drill is overdue.
    DrillOverdue,
    /// An action is stuck and has not progressed.
    ActionStuck,
}

impl SignalKind {
    /// Every kind there is. Walking this ensures a new variant is handled
    /// everywhere it is needed.
    pub const ALL: [Self; 6] = [
        Self::DataplaneHeartbeatStale,
        Self::DeploymentUnreachable,
        Self::BackupMissing,
        Self::BackupFailed,
        Self::DrillOverdue,
        Self::ActionStuck,
    ];

    /// The string name of this kind, used in the database and on the wire.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DataplaneHeartbeatStale => "dataplane_heartbeat_stale",
            Self::DeploymentUnreachable => "deployment_unreachable",
            Self::BackupMissing => "backup_missing",
            Self::BackupFailed => "backup_failed",
            Self::DrillOverdue => "drill_overdue",
            Self::ActionStuck => "action_stuck",
        }
    }
}

impl fmt::Display for SignalKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for SignalKind {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "dataplane_heartbeat_stale" => Ok(Self::DataplaneHeartbeatStale),
            "deployment_unreachable" => Ok(Self::DeploymentUnreachable),
            "backup_missing" => Ok(Self::BackupMissing),
            "backup_failed" => Ok(Self::BackupFailed),
            "drill_overdue" => Ok(Self::DrillOverdue),
            "action_stuck" => Ok(Self::ActionStuck),
            other => Err(CoreError::InternalError(format!(
                "unknown signal kind '{other}'"
            ))),
        }
    }
}

/// What the signal is about.
///
/// Like `SignalKind`, a closed enum enforcing that new subjects are handled
/// everywhere they are added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum SignalSubject {
    /// A data plane.
    Dataplane { id: DataPlaneId },
    /// A deployment.
    Deployment { id: DeploymentId },
    /// An action (a generic UUID, as actions may span multiple bounded contexts).
    Action { id: Uuid },
}

/// A statement about something that happened in the fleet, never revised
/// afterwards.
///
/// A signal with an open row (closed_at IS NULL) for the same dedup_key is
/// updated instead of being inserted: the last_seen_at is refreshed and the
/// message is replaced. This invariant is enforced in the schema via a partial
/// unique index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Signal {
    pub id: SignalId,
    pub kind: SignalKind,
    pub subject: SignalSubject,
    pub dedup_key: String,
    pub message: String,
    pub opened_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

impl Signal {
    /// The only constructor. A signal is opened at a given time and carries
    /// that time as both opened_at and last_seen_at.
    pub fn open(
        id: SignalId,
        kind: SignalKind,
        subject: SignalSubject,
        dedup_key: String,
        message: String,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            kind,
            subject,
            dedup_key,
            message,
            opened_at: at,
            last_seen_at: at,
            closed_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_signal_kind_round_trips_through_its_name() {
        for kind in SignalKind::ALL {
            assert_eq!(
                kind.to_string().parse::<SignalKind>().unwrap(),
                kind,
                "{kind:?}"
            );
        }
    }

    #[test]
    fn no_two_kinds_share_a_name() {
        let mut names: Vec<_> = SignalKind::ALL.iter().map(SignalKind::to_string).collect();
        let before = names.len();
        names.sort();
        names.dedup();

        assert_eq!(names.len(), before);
    }

    #[test]
    fn a_signal_records_the_time_it_was_given() {
        let now = Utc::now();
        let id = SignalId(Uuid::new_v4());
        let subject = SignalSubject::Dataplane {
            id: DataPlaneId(Uuid::new_v4()),
        };

        let signal = Signal::open(
            id,
            SignalKind::DataplaneHeartbeatStale,
            subject,
            "key-1".to_string(),
            "Dataplane is not sending heartbeats".to_string(),
            now,
        );

        assert_eq!(signal.opened_at, now);
        assert_eq!(signal.last_seen_at, now);
        assert!(signal.closed_at.is_none());
    }
}
