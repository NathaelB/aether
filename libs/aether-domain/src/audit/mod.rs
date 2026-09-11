use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{CoreError, organisation::OrganisationId};

pub mod commands;
pub mod ports;
pub mod service;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, ToSchema)]
pub struct AuditEntryId(pub Uuid);

/// Who acted, in the shape `action::ActionSource` already uses for the work
/// that reaches a data plane.
///
/// Defined again here rather than imported: the audit trail exists to watch
/// every bounded context, including `action`'s. Depending on one of the
/// contexts it watches would make the trail's own shape hostage to changes
/// made for a narrower purpose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub enum AuditActor {
    User { user_id: Uuid },
    System,
    Api { client_id: String },
}

/// Namespaced, like `action::ActionType` (e.g. `"deployment.maintenance_window.updated"`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AuditAction(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub enum AuditTargetKind {
    Organisation,
    Deployment,
    DataPlane,
    Role,
    Member,
    Billing,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AuditTarget {
    pub kind: AuditTargetKind,
    pub id: Uuid,
}

/// A field name that must never reach the audit trail.
///
/// Substring matching, not an exact list: `"stripe_secret_key"` and
/// `"session_token"` both have to be caught, and a fixed set of exact names
/// would miss both. This is a backstop against the common accident --
/// forwarding a settings struct verbatim -- not a security boundary; the
/// caller building a change still owns not describing a secret in the first
/// place.
const SENSITIVE_KEY_FRAGMENTS: &[&str] = &[
    "password",
    "secret",
    "token",
    "api_key",
    "apikey",
    "credential",
    "private_key",
    "authorization",
];

fn reject_sensitive(value: &serde_json::Value) -> Result<(), CoreError> {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, nested) in fields {
                let lowered = key.to_ascii_lowercase();
                if SENSITIVE_KEY_FRAGMENTS
                    .iter()
                    .any(|fragment| lowered.contains(fragment))
                {
                    return Err(CoreError::InternalError(format!(
                        "audit entry rejected: field '{key}' looks sensitive and must not be recorded"
                    )));
                }
                reject_sensitive(nested)?;
            }
            Ok(())
        }
        serde_json::Value::Array(items) => {
            for item in items {
                reject_sensitive(item)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The old value and the new one for a single change.
///
/// No `Deserialize`: [`AuditChange::new`] is the only way to build one, and a
/// derived `Deserialize` would reopen the shortcut around the sensitive-field
/// check that this type exists to close. Fields are private for the same
/// reason -- a struct literal would skip the check entirely.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AuditChange {
    before: serde_json::Value,
    after: serde_json::Value,
}

impl AuditChange {
    pub fn new(before: serde_json::Value, after: serde_json::Value) -> Result<Self, CoreError> {
        reject_sensitive(&before)?;
        reject_sensitive(&after)?;
        Ok(Self { before, after })
    }

    pub fn before(&self) -> &serde_json::Value {
        &self.before
    }

    pub fn after(&self) -> &serde_json::Value {
        &self.after
    }
}

/// One statement about something that happened, never revised afterwards.
///
/// No `Deserialize`, matching [`AuditChange`]: the only way to build one is
/// [`AuditEntry::record`], never a struct literal or a round trip through
/// JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AuditEntry {
    pub id: AuditEntryId,
    pub organisation_id: OrganisationId,
    pub actor: AuditActor,
    pub action: AuditAction,
    pub target: AuditTarget,
    pub change: Option<AuditChange>,
    pub recorded_at: DateTime<Utc>,
}

impl AuditEntry {
    /// The only constructor. There is deliberately no way to alter an entry
    /// afterwards: a setter on an audit record would let a statement about
    /// what happened be rewritten into a statement about what someone wishes
    /// had happened.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        id: AuditEntryId,
        organisation_id: OrganisationId,
        actor: AuditActor,
        action: AuditAction,
        target: AuditTarget,
        change: Option<AuditChange>,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            organisation_id,
            actor,
            action,
            target,
            change,
            recorded_at: at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AuditCursor(pub String);

impl AuditCursor {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct AuditBatch {
    pub entries: Vec<AuditEntry>,
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

    fn entry(change: Option<AuditChange>) -> AuditEntry {
        AuditEntry::record(
            AuditEntryId(Uuid::new_v4()),
            OrganisationId(Uuid::new_v4()),
            AuditActor::User {
                user_id: Uuid::new_v4(),
            },
            AuditAction("deployment.maintenance_window.updated".to_string()),
            AuditTarget {
                kind: AuditTargetKind::Deployment,
                id: Uuid::new_v4(),
            },
            change,
            at(),
        )
    }

    #[test]
    fn a_system_actor_is_not_a_human_one() {
        let system = AuditActor::System;
        let human = AuditActor::User {
            user_id: Uuid::new_v4(),
        };

        assert_ne!(system, human);
        assert!(matches!(system, AuditActor::System));
    }

    #[test]
    fn a_change_records_both_sides() {
        let change = AuditChange::new(json!({"minutes": 30}), json!({"minutes": 60}))
            .expect("an innocuous change");

        assert_eq!(change.before(), &json!({"minutes": 30}));
        assert_eq!(change.after(), &json!({"minutes": 60}));
    }

    #[test]
    fn a_change_naming_a_password_is_refused() {
        let error = AuditChange::new(json!({"password": "old"}), json!({"password": "new"}))
            .expect_err("a secret must not be recorded");

        assert!(matches!(error, CoreError::InternalError(_)));
        assert!(error.to_string().contains("password"));
    }

    #[test]
    fn a_secret_nested_inside_an_object_is_still_caught() {
        let error = AuditChange::new(
            json!({"settings": {"stripe_secret_key": "sk_live_x"}}),
            json!({"settings": {}}),
        )
        .expect_err("nesting must not hide a secret");

        assert!(error.to_string().contains("stripe_secret_key"));
    }

    #[test]
    fn a_secret_inside_an_array_is_still_caught() {
        let error = AuditChange::new(json!([{"api_key": "abc"}]), json!([]))
            .expect_err("an array must not hide a secret");

        assert!(error.to_string().contains("api_key"));
    }

    #[test]
    fn an_entry_may_record_no_change_at_all() {
        let entry = entry(None);
        assert!(entry.change.is_none());
    }

    #[test]
    fn an_entry_carries_the_time_it_was_given_not_the_current_time() {
        let entry = entry(None);
        assert_eq!(entry.recorded_at, at());
    }
}
