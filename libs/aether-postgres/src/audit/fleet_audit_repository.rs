use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use aether_domain::CoreError;
use aether_domain::audit::{
    AuditChange, AuditCursor,
    fleet::{
        FleetActor, FleetAuditAction, FleetAuditBatch, FleetAuditEntry, FleetAuditEntryId,
        FleetTarget, ports::FleetAuditRepository,
    },
};
use aether_domain::dataplane::value_objects::DataPlaneId;
use aether_macros::repository;
use aether_persistence::SharedTx;

#[derive(FromRow)]
struct FleetAuditRow {
    id: Uuid,
    actor_type: String,
    actor_subject: Option<String>,
    actor_client_id: Option<String>,
    action: String,
    target_dataplane_id: Option<Uuid>,
    target_subject: Option<String>,
    change_before: Option<serde_json::Value>,
    change_after: Option<serde_json::Value>,
    recorded_at: DateTime<Utc>,
}

impl FleetAuditRow {
    fn into_entry(self) -> Result<FleetAuditEntry, CoreError> {
        let actor = parse_actor(
            &self.actor_type,
            self.actor_subject,
            self.actor_client_id.as_deref(),
        )?;

        let target = parse_target(self.target_dataplane_id, self.target_subject)?;

        let change = match (self.change_before, self.change_after) {
            (Some(before), Some(after)) => Some(AuditChange::new(before, after)?),
            (None, None) => None,
            _ => {
                return Err(CoreError::InternalError(
                    "fleet audit entry has one side of a change but not the other".to_string(),
                ));
            }
        };

        Ok(FleetAuditEntry::record(
            FleetAuditEntryId(self.id),
            actor,
            self.action.parse::<FleetAuditAction>()?,
            target,
            change,
            self.recorded_at,
        ))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[repository(domain = FleetAudit, backend = Postgres)]
pub struct PostgresFleetAuditRepository<'tx> {
    tx: SharedTx<'tx>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'tx> PostgresFleetAuditRepository<'tx> {
    pub fn new(tx: &SharedTx<'tx>) -> Self {
        Self { tx: tx.clone() }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl FleetAuditRepository for PostgresFleetAuditRepository<'_> {
    async fn append(&self, entry: FleetAuditEntry) -> Result<(), CoreError> {
        let (actor_type, actor_subject, actor_client_id) = actor_to_row(&entry.actor);
        let (target_dataplane_id, target_subject) = target_to_row(&entry.target);
        let (change_before, change_after) = match &entry.change {
            Some(change) => (Some(change.before().clone()), Some(change.after().clone())),
            None => (None, None),
        };

        {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            INSERT INTO fleet_audit_log (
                id,
                actor_type,
                actor_subject,
                actor_client_id,
                action,
                target_dataplane_id,
                target_subject,
                change_before,
                change_after,
                recorded_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10
            )
            "#,
                entry.id.0,
                actor_type,
                actor_subject,
                actor_client_id,
                entry.action.to_string(),
                target_dataplane_id,
                target_subject,
                change_before,
                change_after,
                entry.recorded_at,
            )
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to insert fleet audit entry: {e}"),
        })?;

        Ok(())
    }

    async fn list(
        &self,
        cursor: Option<AuditCursor>,
        limit: usize,
    ) -> Result<FleetAuditBatch, CoreError> {
        let rows = if let Some(cursor) = cursor {
            let (cursor_at, cursor_id) = parse_cursor(&cursor)?;
            {
                let mut tx = self.tx.lock().await;
                sqlx::query_as!(
                    FleetAuditRow,
                    r#"
                SELECT id,
                       actor_type,
                       actor_subject,
                       actor_client_id,
                       action,
                       target_dataplane_id,
                       target_subject,
                       change_before,
                       change_after,
                       recorded_at
                FROM fleet_audit_log
                WHERE (recorded_at, id) < ($1, $2)
                ORDER BY recorded_at DESC, id DESC
                LIMIT $3
                "#,
                    cursor_at,
                    cursor_id,
                    limit as i64
                )
                .fetch_all(&mut ***tx)
                .await
            }
            .map_err(|e| CoreError::DatabaseError {
                message: format!("Failed to list fleet audit entries: {e}"),
            })?
        } else {
            {
                let mut tx = self.tx.lock().await;
                sqlx::query_as!(
                    FleetAuditRow,
                    r#"
                SELECT id,
                       actor_type,
                       actor_subject,
                       actor_client_id,
                       action,
                       target_dataplane_id,
                       target_subject,
                       change_before,
                       change_after,
                       recorded_at
                FROM fleet_audit_log
                ORDER BY recorded_at DESC, id DESC
                LIMIT $1
                "#,
                    limit as i64
                )
                .fetch_all(&mut ***tx)
                .await
            }
            .map_err(|e| CoreError::DatabaseError {
                message: format!("Failed to list fleet audit entries: {e}"),
            })?
        };

        let entries = rows
            .into_iter()
            .map(FleetAuditRow::into_entry)
            .collect::<Result<Vec<FleetAuditEntry>, CoreError>>()?;

        let next_cursor = entries.last().map(|entry| {
            AuditCursor::new(format!("{}|{}", entry.recorded_at.to_rfc3339(), entry.id.0))
        });

        Ok(FleetAuditBatch {
            entries,
            next_cursor,
        })
    }
}

fn actor_to_row(actor: &FleetActor) -> (String, Option<String>, Option<String>) {
    match actor {
        FleetActor::Operator { subject } => ("operator".to_string(), Some(subject.clone()), None),
        FleetActor::Api { client_id } => ("api".to_string(), None, Some(client_id.clone())),
        FleetActor::System => ("system".to_string(), None, None),
    }
}

fn parse_actor(
    raw: &str,
    subject: Option<String>,
    client_id: Option<&str>,
) -> Result<FleetActor, CoreError> {
    match raw.to_ascii_lowercase().as_str() {
        "operator" => {
            let subject = subject.ok_or_else(|| {
                CoreError::InternalError(
                    "Missing actor_subject for an operator fleet audit entry".to_string(),
                )
            })?;
            Ok(FleetActor::Operator { subject })
        }
        "api" => {
            let client_id = client_id.ok_or_else(|| {
                CoreError::InternalError(
                    "Missing actor_client_id for an api fleet audit entry".to_string(),
                )
            })?;
            Ok(FleetActor::Api {
                client_id: client_id.to_string(),
            })
        }
        "system" => Ok(FleetActor::System),
        other => Err(CoreError::InternalError(format!(
            "unknown fleet audit actor type '{other}'"
        ))),
    }
}

fn target_to_row(target: &FleetTarget) -> (Option<Uuid>, Option<String>) {
    match target {
        FleetTarget::DataPlane { id } => (Some(id.0), None),
        FleetTarget::Operator { subject } => (None, Some(subject.clone())),
    }
}

/// Exactly one of the two columns, which the table's own check already
/// enforces on the way in.
///
/// Read back defensively all the same: a row that satisfied the check when it
/// was written is not a row that still does after somebody's migration, and an
/// entry naming two targets or none is one the trail cannot honestly display.
fn parse_target(
    dataplane_id: Option<Uuid>,
    subject: Option<String>,
) -> Result<FleetTarget, CoreError> {
    match (dataplane_id, subject) {
        (Some(id), None) => Ok(FleetTarget::DataPlane {
            id: DataPlaneId(id),
        }),
        (None, Some(subject)) => Ok(FleetTarget::Operator { subject }),
        (None, None) => Err(CoreError::InternalError(
            "fleet audit entry names no target".to_string(),
        )),
        (Some(_), Some(_)) => Err(CoreError::InternalError(
            "fleet audit entry names both a data plane and a subject".to_string(),
        )),
    }
}

fn parse_cursor(cursor: &AuditCursor) -> Result<(DateTime<Utc>, Uuid), CoreError> {
    let (timestamp, id) = cursor
        .0
        .split_once('|')
        .ok_or_else(|| CoreError::InternalError("Invalid cursor format".to_string()))?;

    let parsed_at = DateTime::parse_from_rfc3339(timestamp)
        .map_err(|e| CoreError::InternalError(format!("Invalid cursor timestamp: {e}")))?
        .with_timezone(&Utc);

    let parsed_id = Uuid::parse_str(id)
        .map_err(|e| CoreError::InternalError(format!("Invalid cursor id: {e}")))?;

    Ok((parsed_at, parsed_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actor_round_trips_through_its_row_values() {
        let operator = FleetActor::Operator {
            subject: "operator-1".to_string(),
        };
        let (kind, subject, client) = actor_to_row(&operator);
        assert_eq!(
            parse_actor(&kind, subject, client.as_deref()).expect("known"),
            operator
        );

        let api = FleetActor::Api {
            client_id: "client-1".to_string(),
        };
        let (kind, subject, client) = actor_to_row(&api);
        assert_eq!(
            parse_actor(&kind, subject, client.as_deref()).expect("known"),
            api
        );

        let (kind, subject, client) = actor_to_row(&FleetActor::System);
        assert_eq!(
            parse_actor(&kind, subject, client.as_deref()).expect("known"),
            FleetActor::System
        );
    }

    /// The whole reason the actor is a subject here: an operator entry whose
    /// subject went missing would name nobody, and naming nobody is what this
    /// trail exists to prevent.
    #[test]
    fn parse_actor_reports_missing_fields_rather_than_naming_nobody() {
        let err = parse_actor("operator", None, None).unwrap_err();
        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("actor_subject"))
        );

        let err = parse_actor("api", None, None).unwrap_err();
        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("actor_client_id"))
        );
    }

    #[test]
    fn an_unknown_actor_type_is_reported_rather_than_guessed() {
        assert!(parse_actor("user", None, None).is_err());
    }

    #[test]
    fn target_round_trips_through_its_row_values() {
        let cluster = FleetTarget::DataPlane {
            id: DataPlaneId(Uuid::new_v4()),
        };
        let (id, subject) = target_to_row(&cluster);
        assert_eq!(parse_target(id, subject).expect("known"), cluster);

        let operator = FleetTarget::Operator {
            subject: "operator-1".to_string(),
        };
        let (id, subject) = target_to_row(&operator);
        assert_eq!(parse_target(id, subject).expect("known"), operator);
    }

    #[test]
    fn a_row_naming_no_target_or_two_is_refused() {
        assert!(parse_target(None, None).is_err());
        assert!(parse_target(Some(Uuid::new_v4()), Some("s".to_string())).is_err());
    }

    /// Every action name is written by `Display` and read back by `FromStr`,
    /// so a variant whose two halves disagree would be written here and
    /// unreadable on the way out. The domain walks `ALL`; this asserts the
    /// column is what it walks.
    #[test]
    fn every_action_survives_the_column() {
        for action in FleetAuditAction::ALL {
            assert_eq!(
                action
                    .to_string()
                    .parse::<FleetAuditAction>()
                    .expect("known"),
                action
            );
        }
    }

    #[test]
    fn parse_cursor_handles_valid_and_invalid() {
        let cursor = AuditCursor::new("2026-01-01T00:00:00Z|aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let (timestamp, id) = parse_cursor(&cursor).expect("a well formed cursor");

        assert_eq!(
            timestamp,
            DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                .expect("a valid instant")
                .with_timezone(&Utc)
        );
        assert_eq!(
            id,
            Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").expect("a valid uuid")
        );

        assert!(parse_cursor(&AuditCursor::new("bad")).is_err());
        assert!(
            parse_cursor(&AuditCursor::new(
                "not-a-time|aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"
            ))
            .is_err()
        );
        assert!(parse_cursor(&AuditCursor::new("2026-01-01T00:00:00Z|not-a-uuid")).is_err());
    }
}
