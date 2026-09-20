//! The installation's own trail.
//!
//! [`super`] records what happened inside an organisation, and every read of
//! it is scoped to one. Fleet actions have no organisation: registering a data
//! plane, draining one, granting somebody a platform right are acts against
//! the installation, and filing them under a tenant would put an
//! installation-wide decision in some customer's trail.
//!
//! So they live here, in their own table behind their own port. Not the
//! organisation trail with a nullable scope: every query there filters by
//! organisation, and a nullable column on such a table is one forgotten
//! `WHERE` away from leaking fleet actions to a customer. Nothing in this
//! module has an organisation to filter on, which is what makes that mistake
//! unavailable rather than merely avoided.

use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    CoreError,
    audit::{AuditChange, AuditCursor},
    dataplane::value_objects::DataPlaneId,
};

pub mod commands;
pub mod ports;
pub mod service;

/// A trail the services' own tests can read back. See the module for why it is
/// shared rather than written per bounded context.
#[cfg(test)]
pub mod fixtures;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, ToSchema)]
pub struct FleetAuditEntryId(pub Uuid);

/// Who acted on the fleet.
///
/// Named by the subject the identity provider issued, where
/// [`super::AuditActor`] resolves a user to its `users` row. Two reasons, and
/// the second is the one that decided it.
///
/// Platform rights are keyed on subject: `platform_operators.subject` is what
/// the installation actually decides with, so it is the honest name for
/// whoever exercised one.
///
/// And a lookup can come back empty. [`super::service::audit_actor`] refuses
/// an identity it cannot resolve, deliberately -- an organisation entry that
/// cannot say who acted is worse than none. Here that refusal would travel
/// outward: draining a cluster would fail because the trail could not name the
/// operator, which trades an outage for a bookkeeping gap. Recording must not
/// be able to stop the act it records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum FleetActor {
    Operator {
        subject: String,
    },
    Api {
        client_id: String,
    },
    /// The platform acting on its own -- the first operator an installation
    /// names at startup, and nothing else so far.
    System,
}

/// One thing an operator did to the installation.
///
/// An enum where [`super::AuditAction`] is a string. That one is open because
/// every bounded context writes into it and the set has no owner; this one is
/// closed and the installation owns it, so a name is a variant here and the
/// test below walks [`FleetAuditAction::ALL`] -- an action added without a
/// name never reaches the database.
///
/// Every variant is renamed explicitly rather than left to `rename_all`,
/// which would derive `data_plane_registered` from the variant while
/// [`fmt::Display`] writes `dataplane.registered` into the column. Two names
/// for one action is not a cosmetic difference: somebody filtering a trail
/// reads the JSON name and greps the database for it. The test below asserts
/// the two agree for every variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, ToSchema)]
pub enum FleetAuditAction {
    /// A cluster joined the fleet.
    #[serde(rename = "dataplane.registered")]
    DataPlaneRegistered,
    /// No new work placed here; what runs stays.
    #[serde(rename = "dataplane.drained")]
    DataPlaneDrained,
    /// Out of service.
    #[serde(rename = "dataplane.disabled")]
    DataPlaneDisabled,
    /// Put back on the path it was on.
    #[serde(rename = "dataplane.returned_to_service")]
    DataPlaneReturnedToService,
    /// A cluster was given a new credential, which makes the previous one
    /// stop working. One of the two acts that take a customer's deployment
    /// offline without touching the deployment.
    #[serde(rename = "dataplane.credential_reissued")]
    DataPlaneCredentialReissued,
    /// A subject was granted platform rights, or had the ones it held
    /// changed.
    #[serde(rename = "operator.granted")]
    OperatorGranted,
    #[serde(rename = "operator.revoked")]
    OperatorRevoked,
}

impl FleetAuditAction {
    /// Every action there is.
    ///
    /// Walked by the round trip test below, the same way
    /// [`crate::platform::PlatformRight::ALL`] is: an action with no name is
    /// one the trail silently drops.
    pub const ALL: [Self; 7] = [
        Self::DataPlaneRegistered,
        Self::DataPlaneDrained,
        Self::DataPlaneDisabled,
        Self::DataPlaneReturnedToService,
        Self::DataPlaneCredentialReissued,
        Self::OperatorGranted,
        Self::OperatorRevoked,
    ];
}

impl fmt::Display for FleetAuditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::DataPlaneRegistered => "dataplane.registered",
            Self::DataPlaneDrained => "dataplane.drained",
            Self::DataPlaneDisabled => "dataplane.disabled",
            Self::DataPlaneReturnedToService => "dataplane.returned_to_service",
            Self::DataPlaneCredentialReissued => "dataplane.credential_reissued",
            Self::OperatorGranted => "operator.granted",
            Self::OperatorRevoked => "operator.revoked",
        };

        write!(f, "{name}")
    }
}

impl FromStr for FleetAuditAction {
    type Err = CoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "dataplane.registered" => Ok(Self::DataPlaneRegistered),
            "dataplane.drained" => Ok(Self::DataPlaneDrained),
            "dataplane.disabled" => Ok(Self::DataPlaneDisabled),
            "dataplane.returned_to_service" => Ok(Self::DataPlaneReturnedToService),
            "dataplane.credential_reissued" => Ok(Self::DataPlaneCredentialReissued),
            "operator.granted" => Ok(Self::OperatorGranted),
            "operator.revoked" => Ok(Self::OperatorRevoked),
            other => Err(CoreError::InternalError(format!(
                "unknown fleet audit action '{other}'"
            ))),
        }
    }
}

/// What an entry was recorded against.
///
/// An enum rather than the `(kind, Uuid)` pair [`super::AuditTarget`] uses: an
/// operator is named by a subject, and a subject is not a UUID. Carrying one
/// in a UUID column would mean inventing an identifier for a row the identity
/// provider owns, and inventing one is how two subjects eventually collide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum FleetTarget {
    DataPlane { id: DataPlaneId },
    Operator { subject: String },
}

/// One statement about something that happened to the installation, never
/// revised afterwards.
///
/// No `organisation_id`, and not an `Option<OrganisationId>` either: a field
/// that is always `None` is a column somebody eventually fills. No
/// `Deserialize`, matching [`super::AuditEntry`] -- [`FleetAuditEntry::record`]
/// is the only way to build one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct FleetAuditEntry {
    pub id: FleetAuditEntryId,
    pub actor: FleetActor,
    pub action: FleetAuditAction,
    pub target: FleetTarget,
    pub change: Option<AuditChange>,
    pub recorded_at: DateTime<Utc>,
}

impl FleetAuditEntry {
    /// The only constructor, and there is no way to alter an entry afterwards.
    /// A setter on an audit record would let a statement about what happened
    /// be rewritten into a statement about what somebody wishes had happened.
    pub fn record(
        id: FleetAuditEntryId,
        actor: FleetActor,
        action: FleetAuditAction,
        target: FleetTarget,
        change: Option<AuditChange>,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            actor,
            action,
            target,
            change,
            recorded_at: at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct FleetAuditBatch {
    pub entries: Vec<FleetAuditEntry>,
    pub next_cursor: Option<AuditCursor>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .expect("a valid instant")
            .with_timezone(&Utc)
    }

    /// An action with no name is one nobody can record, and one that does not
    /// survive the round trip is one the database silently drops.
    #[test]
    fn every_action_round_trips_through_its_name() {
        for action in FleetAuditAction::ALL {
            assert_eq!(
                action.to_string().parse::<FleetAuditAction>().unwrap(),
                action
            );
        }
    }

    /// The column and the wire say the same thing. They are written by two
    /// different mechanisms -- `Display` and serde -- and nothing but this
    /// keeps them from drifting the moment a variant is added.
    #[test]
    fn the_name_in_the_column_is_the_name_on_the_wire() {
        for action in FleetAuditAction::ALL {
            let on_the_wire = serde_json::to_value(action).expect("serialisable");

            assert_eq!(
                on_the_wire,
                serde_json::Value::String(action.to_string()),
                "{action:?}"
            );
        }
    }

    #[test]
    fn a_name_nobody_records_is_refused_rather_than_guessed() {
        assert!("dataplane.deleted".parse::<FleetAuditAction>().is_err());
        assert!("".parse::<FleetAuditAction>().is_err());
    }

    /// Every action is named for the thing it was done to, and the trail is
    /// read by scanning that column. Two actions sharing a name would merge
    /// two different acts into one line.
    #[test]
    fn no_two_actions_share_a_name() {
        let mut names: Vec<_> = FleetAuditAction::ALL
            .iter()
            .map(FleetAuditAction::to_string)
            .collect();
        let before = names.len();
        names.sort();
        names.dedup();

        assert_eq!(names.len(), before);
    }

    #[test]
    fn an_entry_carries_the_time_it_was_given_not_the_current_time() {
        let entry = FleetAuditEntry::record(
            FleetAuditEntryId(Uuid::new_v4()),
            FleetActor::System,
            FleetAuditAction::DataPlaneRegistered,
            FleetTarget::DataPlane {
                id: DataPlaneId(Uuid::new_v4()),
            },
            None,
            at(),
        );

        assert_eq!(entry.recorded_at, at());
    }

    /// The backstop [`AuditChange`] carries is the reason it is reused here
    /// rather than reinvented: a credential re-issue is recorded beside the
    /// secret it replaced, and the secret must not travel with it.
    #[test]
    fn a_change_naming_a_secret_is_refused_here_too() {
        assert!(AuditChange::new(json!({"herald_secret": "x"}), json!({})).is_err());
    }
}
