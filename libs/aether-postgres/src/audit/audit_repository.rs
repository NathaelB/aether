use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use aether_domain::CoreError;
use aether_domain::audit::{
    AuditAction, AuditActor, AuditBatch, AuditChange, AuditCursor, AuditEntry, AuditEntryId,
    AuditTarget, AuditTargetKind, ports::AuditRepository,
};
use aether_domain::organisation::OrganisationId;
use aether_macros::repository;
use aether_persistence::SharedTx;

#[derive(FromRow)]
struct AuditRow {
    id: Uuid,
    organisation_id: Uuid,
    actor_type: String,
    actor_user_id: Option<Uuid>,
    actor_client_id: Option<String>,
    action: String,
    target_kind: String,
    target_id: Uuid,
    change_before: Option<serde_json::Value>,
    change_after: Option<serde_json::Value>,
    recorded_at: DateTime<Utc>,
}

impl AuditRow {
    fn into_entry(self) -> Result<AuditEntry, CoreError> {
        let actor = parse_actor(
            &self.actor_type,
            self.actor_user_id,
            self.actor_client_id.as_deref(),
        )?;

        let change = match (self.change_before, self.change_after) {
            (Some(before), Some(after)) => Some(AuditChange::new(before, after)?),
            (None, None) => None,
            _ => {
                return Err(CoreError::InternalError(
                    "audit entry has one side of a change but not the other".to_string(),
                ));
            }
        };

        Ok(AuditEntry::record(
            AuditEntryId(self.id),
            OrganisationId(self.organisation_id),
            actor,
            AuditAction(self.action),
            AuditTarget {
                kind: parse_target_kind(&self.target_kind),
                id: self.target_id,
            },
            change,
            self.recorded_at,
        ))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[repository(domain = Audit, backend = Postgres)]
pub struct PostgresAuditRepository<'tx> {
    tx: SharedTx<'tx>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'tx> PostgresAuditRepository<'tx> {
    pub fn new(tx: &SharedTx<'tx>) -> Self {
        Self { tx: tx.clone() }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl AuditRepository for PostgresAuditRepository<'_> {
    async fn append(&self, entry: AuditEntry) -> Result<(), CoreError> {
        let (actor_type, actor_user_id, actor_client_id) = actor_to_row(&entry.actor);
        let (change_before, change_after) = match &entry.change {
            Some(change) => (Some(change.before().clone()), Some(change.after().clone())),
            None => (None, None),
        };

        {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            INSERT INTO audit_log (
                id,
                organisation_id,
                actor_type,
                actor_user_id,
                actor_client_id,
                action,
                target_kind,
                target_id,
                change_before,
                change_after,
                recorded_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11
            )
            "#,
                entry.id.0,
                entry.organisation_id.0,
                actor_type,
                actor_user_id,
                actor_client_id,
                entry.action.0,
                target_kind_to_string(&entry.target.kind),
                entry.target.id,
                change_before,
                change_after,
                entry.recorded_at,
            )
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to insert audit entry: {e}"),
        })?;

        Ok(())
    }

    async fn list_for_organisation(
        &self,
        organisation_id: OrganisationId,
        cursor: Option<AuditCursor>,
        limit: usize,
    ) -> Result<AuditBatch, CoreError> {
        let rows = if let Some(cursor) = cursor {
            let (cursor_at, cursor_id) = parse_cursor(&cursor)?;
            {
                let mut tx = self.tx.lock().await;
                sqlx::query_as!(
                    AuditRow,
                    r#"
                SELECT id,
                       organisation_id,
                       actor_type,
                       actor_user_id,
                       actor_client_id,
                       action,
                       target_kind,
                       target_id,
                       change_before,
                       change_after,
                       recorded_at
                FROM audit_log
                WHERE organisation_id = $1
                  AND (recorded_at, id) < ($2, $3)
                ORDER BY recorded_at DESC, id DESC
                LIMIT $4
                "#,
                    organisation_id.0,
                    cursor_at,
                    cursor_id,
                    limit as i64
                )
                .fetch_all(&mut ***tx)
                .await
            }
            .map_err(|e| CoreError::DatabaseError {
                message: format!("Failed to list audit entries: {e}"),
            })?
        } else {
            {
                let mut tx = self.tx.lock().await;
                sqlx::query_as!(
                    AuditRow,
                    r#"
                SELECT id,
                       organisation_id,
                       actor_type,
                       actor_user_id,
                       actor_client_id,
                       action,
                       target_kind,
                       target_id,
                       change_before,
                       change_after,
                       recorded_at
                FROM audit_log
                WHERE organisation_id = $1
                ORDER BY recorded_at DESC, id DESC
                LIMIT $2
                "#,
                    organisation_id.0,
                    limit as i64
                )
                .fetch_all(&mut ***tx)
                .await
            }
            .map_err(|e| CoreError::DatabaseError {
                message: format!("Failed to list audit entries: {e}"),
            })?
        };

        let entries = rows
            .into_iter()
            .map(|row| row.into_entry())
            .collect::<Result<Vec<AuditEntry>, CoreError>>()?;

        let next_cursor = entries.last().map(|entry| {
            AuditCursor::new(format!("{}|{}", entry.recorded_at.to_rfc3339(), entry.id.0))
        });

        Ok(AuditBatch {
            entries,
            next_cursor,
        })
    }
}

fn actor_to_row(actor: &AuditActor) -> (String, Option<Uuid>, Option<String>) {
    match actor {
        AuditActor::User { user_id } => ("user".to_string(), Some(*user_id), None),
        AuditActor::System => ("system".to_string(), None, None),
        AuditActor::Api { client_id } => ("api".to_string(), None, Some(client_id.clone())),
    }
}

fn parse_actor(
    raw: &str,
    user_id: Option<Uuid>,
    client_id: Option<&str>,
) -> Result<AuditActor, CoreError> {
    match raw.to_ascii_lowercase().as_str() {
        "user" => {
            let user_id = user_id.ok_or_else(|| {
                CoreError::InternalError("Missing actor_user_id for a user audit entry".to_string())
            })?;
            Ok(AuditActor::User { user_id })
        }
        "system" => Ok(AuditActor::System),
        "api" => {
            let client_id = client_id.ok_or_else(|| {
                CoreError::InternalError(
                    "Missing actor_client_id for an api audit entry".to_string(),
                )
            })?;
            Ok(AuditActor::Api {
                client_id: client_id.to_string(),
            })
        }
        other => Err(CoreError::InternalError(format!(
            "unknown audit actor type '{other}'"
        ))),
    }
}

fn target_kind_to_string(kind: &AuditTargetKind) -> String {
    match kind {
        AuditTargetKind::Organisation => "organisation".to_string(),
        AuditTargetKind::Deployment => "deployment".to_string(),
        AuditTargetKind::DataPlane => "dataplane".to_string(),
        AuditTargetKind::Role => "role".to_string(),
        AuditTargetKind::Member => "member".to_string(),
        AuditTargetKind::Billing => "billing".to_string(),
        AuditTargetKind::Custom(value) => value.clone(),
    }
}

fn parse_target_kind(raw: &str) -> AuditTargetKind {
    match raw.to_ascii_lowercase().as_str() {
        "organisation" => AuditTargetKind::Organisation,
        "deployment" => AuditTargetKind::Deployment,
        "dataplane" => AuditTargetKind::DataPlane,
        "role" => AuditTargetKind::Role,
        "member" => AuditTargetKind::Member,
        "billing" => AuditTargetKind::Billing,
        other => AuditTargetKind::Custom(other.to_string()),
    }
}

fn parse_cursor(cursor: &AuditCursor) -> Result<(DateTime<Utc>, Uuid), CoreError> {
    let mut parts = cursor.0.splitn(2, '|');
    let timestamp = parts
        .next()
        .ok_or_else(|| CoreError::InternalError("Invalid cursor format".to_string()))?;
    let id = parts
        .next()
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
    fn actor_round_trips_through_its_row_value() {
        let user_id = Uuid::new_v4();
        let (kind, user, client) = actor_to_row(&AuditActor::User { user_id });
        assert_eq!(
            parse_actor(&kind, user, client.as_deref()).expect("known"),
            AuditActor::User { user_id }
        );

        let (kind, user, client) = actor_to_row(&AuditActor::System);
        assert_eq!(
            parse_actor(&kind, user, client.as_deref()).expect("known"),
            AuditActor::System
        );

        let (kind, user, client) = actor_to_row(&AuditActor::Api {
            client_id: "client-1".to_string(),
        });
        assert_eq!(
            parse_actor(&kind, user, client.as_deref()).expect("known"),
            AuditActor::Api {
                client_id: "client-1".to_string()
            }
        );
    }

    #[test]
    fn parse_actor_reports_missing_fields() {
        let err = parse_actor("user", None, None).unwrap_err();
        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("actor_user_id"))
        );

        let err = parse_actor("api", None, None).unwrap_err();
        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("actor_client_id"))
        );
    }

    #[test]
    fn an_unknown_actor_type_is_reported_rather_than_guessed() {
        assert!(parse_actor("robot", None, None).is_err());
    }

    #[test]
    fn target_kind_round_trips() {
        for kind in [
            AuditTargetKind::Organisation,
            AuditTargetKind::Deployment,
            AuditTargetKind::DataPlane,
            AuditTargetKind::Role,
            AuditTargetKind::Member,
            AuditTargetKind::Billing,
        ] {
            assert_eq!(
                parse_target_kind(&target_kind_to_string(&kind)),
                kind,
                "{kind:?}"
            );
        }

        assert_eq!(
            parse_target_kind("addon"),
            AuditTargetKind::Custom("addon".to_string())
        );
    }

    #[test]
    fn parse_cursor_handles_valid_and_invalid() {
        let cursor = AuditCursor::new("2025-01-01T00:00:00Z|aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let (timestamp, id) = parse_cursor(&cursor).unwrap();

        assert_eq!(
            timestamp,
            DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        );
        assert_eq!(
            id,
            Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap()
        );

        assert!(parse_cursor(&AuditCursor::new("bad")).is_err());
        assert!(
            parse_cursor(&AuditCursor::new(
                "not-a-time|aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"
            ))
            .is_err()
        );
        assert!(parse_cursor(&AuditCursor::new("2025-01-01T00:00:00Z|not-a-uuid")).is_err());
    }
}
