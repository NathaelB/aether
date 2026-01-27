#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use aether_domain::CoreError;
use aether_domain::action::{
    Action, ActionBatch, ActionConstraints, ActionCursor, ActionFailureReason, ActionId,
    ActionMetadata, ActionPayload, ActionSource, ActionStatus, ActionTarget, ActionType,
    ActionVersion, TargetKind, ports::ActionRepository,
};
use aether_domain::deployments::DeploymentId;
use aether_persistence::{PgExecutor, PgTransaction};

#[derive(FromRow)]
struct ActionRow {
    id: Uuid,
    action_type: String,
    target_kind: String,
    target_id: Uuid,
    payload: serde_json::Value,
    version: i32,
    status: String,
    status_at: Option<DateTime<Utc>>,
    status_agent_id: Option<String>,
    status_reason: Option<String>,
    source_type: String,
    source_user_id: Option<Uuid>,
    source_client_id: Option<String>,
    constraints_not_after: Option<DateTime<Utc>>,
    constraints_priority: Option<i16>,
    created_at: DateTime<Utc>,
}

impl ActionRow {
    fn into_action(self) -> Result<Action, CoreError> {
        let target_kind = parse_target_kind(&self.target_kind);
        let target = ActionTarget {
            kind: target_kind,
            id: self.target_id,
        };

        let status = parse_status(
            &self.status,
            self.status_at,
            self.status_agent_id.as_deref(),
            self.status_reason.as_deref(),
        )?;

        let source = parse_source(
            &self.source_type,
            self.source_user_id,
            self.source_client_id.as_deref(),
        )?;

        let priority = match self.constraints_priority {
            Some(value) => Some(u8::try_from(value).map_err(|_| {
                CoreError::InternalError(format!("Invalid action priority value: {}", value))
            })?),
            None => None,
        };

        Ok(Action {
            id: ActionId(self.id),
            action_type: ActionType(self.action_type),
            target,
            payload: ActionPayload { data: self.payload },
            version: ActionVersion(u32::try_from(self.version).map_err(|_| {
                CoreError::InternalError(format!("Invalid action version value: {}", self.version))
            })?),
            status,
            metadata: ActionMetadata {
                source,
                created_at: self.created_at,
                constraints: ActionConstraints {
                    not_after: self.constraints_not_after,
                    priority,
                },
            },
        })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub struct PostgresActionRepository<'e, 't> {
    executor: PgExecutor<'e, 't>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'e, 't> PostgresActionRepository<'e, 't> {
    pub fn new(executor: PgExecutor<'e, 't>) -> Self {
        Self { executor }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn from_tx(tx: &'e PgTransaction<'t>) -> Self {
        Self::new(PgExecutor::from_tx(tx))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'e> PostgresActionRepository<'e, 'e> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn from_pool(pool: &'e sqlx::PgPool) -> Self {
        Self::new(PgExecutor::from_pool(pool))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ActionRepository for PostgresActionRepository<'_, '_> {
    #[cfg_attr(coverage_nightly, coverage(off))]
    async fn append(&self, action: Action) -> Result<(), CoreError> {
        let deployment_id = deployment_id_from_action(&action)?;
        let (status, status_at, status_agent_id, status_reason) = status_to_row(&action.status);
        let (source_type, source_user_id, source_client_id) =
            source_to_row(&action.metadata.source);

        match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query!(
                    r#"
                    INSERT INTO actions (
                        id,
                        deployment_id,
                        action_type,
                        target_kind,
                        target_id,
                        payload,
                        version,
                        status,
                        status_at,
                        status_agent_id,
                        status_reason,
                        source_type,
                        source_user_id,
                        source_client_id,
                        constraints_not_after,
                        constraints_priority,
                        created_at
                    )
                    VALUES (
                        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17
                    )
                    "#,
                    action.id.0,
                    deployment_id.0,
                    action.action_type.0,
                    target_kind_to_string(&action.target.kind),
                    action.target.id,
                    action.payload.data,
                    i32::try_from(action.version.0).map_err(|_| CoreError::InternalError(
                        format!("Invalid action version value: {}", action.version.0)
                    ))?,
                    status,
                    status_at,
                    status_agent_id,
                    status_reason,
                    source_type,
                    source_user_id,
                    source_client_id,
                    action.metadata.constraints.not_after,
                    action
                        .metadata
                        .constraints
                        .priority
                        .map(|value| value as i16),
                    action.metadata.created_at,
                )
                .execute(*pool)
                .await
            }
            PgExecutor::Tx(tx) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query!(
                    r#"
                    INSERT INTO actions (
                        id,
                        deployment_id,
                        action_type,
                        target_kind,
                        target_id,
                        payload,
                        version,
                        status,
                        status_at,
                        status_agent_id,
                        status_reason,
                        source_type,
                        source_user_id,
                        source_client_id,
                        constraints_not_after,
                        constraints_priority,
                        created_at
                    )
                    VALUES (
                        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17
                    )
                    "#,
                    action.id.0,
                    deployment_id.0,
                    action.action_type.0,
                    target_kind_to_string(&action.target.kind),
                    action.target.id,
                    action.payload.data,
                    i32::try_from(action.version.0).map_err(|_| CoreError::InternalError(
                        format!("Invalid action version value: {}", action.version.0)
                    ))?,
                    status,
                    status_at,
                    status_agent_id,
                    status_reason,
                    source_type,
                    source_user_id,
                    source_client_id,
                    action.metadata.constraints.not_after,
                    action
                        .metadata
                        .constraints
                        .priority
                        .map(|value| value as i16),
                    action.metadata.created_at,
                )
                .execute(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to insert action: {}", e),
        })?;

        Ok(())
    }

    async fn get_by_id(
        &self,
        deployment_id: DeploymentId,
        action_id: ActionId,
    ) -> Result<Option<Action>, CoreError> {
        let row = match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query_as!(
                    ActionRow,
                    r#"
                    SELECT id,
                           action_type,
                           target_kind,
                           target_id,
                           payload,
                           version,
                           status,
                           status_at,
                           status_agent_id,
                           status_reason,
                           source_type,
                           source_user_id,
                           source_client_id,
                           constraints_not_after,
                           constraints_priority,
                           created_at
                    FROM actions
                    WHERE deployment_id = $1
                      AND id = $2
                    "#,
                    deployment_id.0,
                    action_id.0
                )
                .fetch_optional(*pool)
                .await
            }
            PgExecutor::Tx(tx) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query_as!(
                    ActionRow,
                    r#"
                    SELECT id,
                           action_type,
                           target_kind,
                           target_id,
                           payload,
                           version,
                           status,
                           status_at,
                           status_agent_id,
                           status_reason,
                           source_type,
                           source_user_id,
                           source_client_id,
                           constraints_not_after,
                           constraints_priority,
                           created_at
                    FROM actions
                    WHERE deployment_id = $1
                      AND id = $2
                    "#,
                    deployment_id.0,
                    action_id.0
                )
                .fetch_optional(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to get action: {}", e),
        })?;

        row.map(|row| row.into_action()).transpose()
    }

    async fn list(
        &self,
        deployment_id: DeploymentId,
        cursor: Option<ActionCursor>,
        limit: usize,
    ) -> Result<ActionBatch, CoreError> {
        let rows = if let Some(cursor) = cursor {
            let (cursor_at, cursor_id) = parse_cursor(&cursor)?;
            match &self.executor {
                PgExecutor::Pool(pool) => {
                    sqlx::query_as!(
                        ActionRow,
                        r#"
                        SELECT id,
                               action_type,
                               target_kind,
                               target_id,
                               payload,
                               version,
                               status,
                               status_at,
                               status_agent_id,
                               status_reason,
                               source_type,
                               source_user_id,
                               source_client_id,
                               constraints_not_after,
                               constraints_priority,
                               created_at
                        FROM actions
                        WHERE deployment_id = $1
                          AND (created_at, id) > ($2, $3)
                        ORDER BY created_at ASC, id ASC
                        LIMIT $4
                        "#,
                        deployment_id.0,
                        cursor_at,
                        cursor_id,
                        limit as i64
                    )
                    .fetch_all(*pool)
                    .await
                }
                PgExecutor::Tx(tx) => {
                    let mut guard = tx.lock().await;
                    let transaction = guard.as_mut().ok_or_else(|| {
                        CoreError::InternalError("Transaction missing".to_string())
                    })?;
                    sqlx::query_as!(
                        ActionRow,
                        r#"
                        SELECT id,
                               action_type,
                               target_kind,
                               target_id,
                               payload,
                               version,
                               status,
                               status_at,
                               status_agent_id,
                               status_reason,
                               source_type,
                               source_user_id,
                               source_client_id,
                               constraints_not_after,
                               constraints_priority,
                               created_at
                        FROM actions
                        WHERE deployment_id = $1
                          AND (created_at, id) > ($2, $3)
                        ORDER BY created_at ASC, id ASC
                        LIMIT $4
                        "#,
                        deployment_id.0,
                        cursor_at,
                        cursor_id,
                        limit as i64
                    )
                    .fetch_all(transaction.as_mut())
                    .await
                }
            }
            .map_err(|e| CoreError::DatabaseError {
                message: format!("Failed to list actions: {}", e),
            })?
        } else {
            match &self.executor {
                PgExecutor::Pool(pool) => {
                    sqlx::query_as!(
                        ActionRow,
                        r#"
                        SELECT id,
                               action_type,
                               target_kind,
                               target_id,
                               payload,
                               version,
                               status,
                               status_at,
                               status_agent_id,
                               status_reason,
                               source_type,
                               source_user_id,
                               source_client_id,
                               constraints_not_after,
                               constraints_priority,
                               created_at
                        FROM actions
                        WHERE deployment_id = $1
                        ORDER BY created_at ASC, id ASC
                        LIMIT $2
                        "#,
                        deployment_id.0,
                        limit as i64
                    )
                    .fetch_all(*pool)
                    .await
                }
                PgExecutor::Tx(tx) => {
                    let mut guard = tx.lock().await;
                    let transaction = guard.as_mut().ok_or_else(|| {
                        CoreError::InternalError("Transaction missing".to_string())
                    })?;
                    sqlx::query_as!(
                        ActionRow,
                        r#"
                        SELECT id,
                               action_type,
                               target_kind,
                               target_id,
                               payload,
                               version,
                               status,
                               status_at,
                               status_agent_id,
                               status_reason,
                               source_type,
                               source_user_id,
                               source_client_id,
                               constraints_not_after,
                               constraints_priority,
                               created_at
                        FROM actions
                        WHERE deployment_id = $1
                        ORDER BY created_at ASC, id ASC
                        LIMIT $2
                        "#,
                        deployment_id.0,
                        limit as i64
                    )
                    .fetch_all(transaction.as_mut())
                    .await
                }
            }
            .map_err(|e| CoreError::DatabaseError {
                message: format!("Failed to list actions: {}", e),
            })?
        };

        let actions = rows
            .into_iter()
            .map(|row| row.into_action())
            .collect::<Result<Vec<Action>, CoreError>>()?;

        let next_cursor = actions.last().map(|action| {
            ActionCursor::new(format!(
                "{}|{}",
                action.metadata.created_at.to_rfc3339(),
                action.id.0
            ))
        });

        Ok(ActionBatch {
            actions,
            next_cursor,
        })
    }
}

fn deployment_id_from_action(action: &Action) -> Result<DeploymentId, CoreError> {
    match action.target.kind {
        TargetKind::Deployment => Ok(DeploymentId(action.target.id)),
        _ => Err(CoreError::InternalError(
            "Action target must be a deployment to persist actions".to_string(),
        )),
    }
}

fn status_to_row(
    status: &ActionStatus,
) -> (
    String,
    Option<DateTime<Utc>>,
    Option<String>,
    Option<String>,
) {
    match status {
        ActionStatus::Pending => ("pending".to_string(), None, None, None),
        ActionStatus::Pulled { agent_id, at } => (
            "pulled".to_string(),
            Some(*at),
            Some(agent_id.clone()),
            None,
        ),
        ActionStatus::Published { at } => ("published".to_string(), Some(*at), None, None),
        ActionStatus::Failed { reason, at } => (
            "failed".to_string(),
            Some(*at),
            None,
            Some(failure_reason_to_string(reason)),
        ),
    }
}

fn parse_status(
    raw: &str,
    status_at: Option<DateTime<Utc>>,
    status_agent_id: Option<&str>,
    status_reason: Option<&str>,
) -> Result<ActionStatus, CoreError> {
    match raw.to_ascii_lowercase().as_str() {
        "pending" => Ok(ActionStatus::Pending),
        "pulled" => {
            let at = status_at.ok_or_else(|| {
                CoreError::InternalError("Missing status_at for pulled action".to_string())
            })?;
            let agent_id = status_agent_id.ok_or_else(|| {
                CoreError::InternalError("Missing status_agent_id for pulled action".to_string())
            })?;
            Ok(ActionStatus::Pulled {
                agent_id: agent_id.to_string(),
                at,
            })
        }
        "published" => {
            let at = status_at.ok_or_else(|| {
                CoreError::InternalError("Missing status_at for published action".to_string())
            })?;
            Ok(ActionStatus::Published { at })
        }
        "failed" => {
            let at = status_at.ok_or_else(|| {
                CoreError::InternalError("Missing status_at for failed action".to_string())
            })?;
            let reason = status_reason.ok_or_else(|| {
                CoreError::InternalError("Missing status_reason for failed action".to_string())
            })?;
            Ok(ActionStatus::Failed {
                reason: parse_failure_reason(reason),
                at,
            })
        }
        other => Err(CoreError::InternalError(format!(
            "Unknown action status: {}",
            other
        ))),
    }
}

fn failure_reason_to_string(reason: &ActionFailureReason) -> String {
    match reason {
        ActionFailureReason::InvalidPayload => "invalid_payload".to_string(),
        ActionFailureReason::UnsupportedAction => "unsupported_action".to_string(),
        ActionFailureReason::PublishFailed => "publish_failed".to_string(),
        ActionFailureReason::Timeout => "timeout".to_string(),
        ActionFailureReason::InternalError(message) => format!("internal:{}", message),
    }
}

fn parse_failure_reason(raw: &str) -> ActionFailureReason {
    match raw {
        "invalid_payload" => ActionFailureReason::InvalidPayload,
        "unsupported_action" => ActionFailureReason::UnsupportedAction,
        "publish_failed" => ActionFailureReason::PublishFailed,
        "timeout" => ActionFailureReason::Timeout,
        value if value.starts_with("internal:") => {
            ActionFailureReason::InternalError(value.trim_start_matches("internal:").to_string())
        }
        other => ActionFailureReason::InternalError(other.to_string()),
    }
}

fn source_to_row(source: &ActionSource) -> (String, Option<Uuid>, Option<String>) {
    match source {
        ActionSource::User { user_id } => ("user".to_string(), Some(*user_id), None),
        ActionSource::System => ("system".to_string(), None, None),
        ActionSource::Api { client_id } => ("api".to_string(), None, Some(client_id.clone())),
    }
}

fn parse_source(
    raw: &str,
    user_id: Option<Uuid>,
    client_id: Option<&str>,
) -> Result<ActionSource, CoreError> {
    match raw.to_ascii_lowercase().as_str() {
        "user" => {
            let user_id = user_id.ok_or_else(|| {
                CoreError::InternalError("Missing source_user_id for action".to_string())
            })?;
            Ok(ActionSource::User { user_id })
        }
        "system" => Ok(ActionSource::System),
        "api" => {
            let client_id = client_id.ok_or_else(|| {
                CoreError::InternalError("Missing source_client_id for action".to_string())
            })?;
            Ok(ActionSource::Api {
                client_id: client_id.to_string(),
            })
        }
        other => Err(CoreError::InternalError(format!(
            "Unknown action source type: {}",
            other
        ))),
    }
}

fn target_kind_to_string(kind: &TargetKind) -> String {
    match kind {
        TargetKind::Deployment => "deployment".to_string(),
        TargetKind::Realm => "realm".to_string(),
        TargetKind::Database => "database".to_string(),
        TargetKind::User => "user".to_string(),
        TargetKind::Custom(value) => value.clone(),
    }
}

fn parse_target_kind(raw: &str) -> TargetKind {
    match raw.to_ascii_lowercase().as_str() {
        "deployment" => TargetKind::Deployment,
        "realm" => TargetKind::Realm,
        "database" => TargetKind::Database,
        "user" => TargetKind::User,
        other => TargetKind::Custom(other.to_string()),
    }
}

fn parse_cursor(cursor: &ActionCursor) -> Result<(DateTime<Utc>, Uuid), CoreError> {
    let mut parts = cursor.0.splitn(2, '|');
    let timestamp = parts
        .next()
        .ok_or_else(|| CoreError::InternalError("Invalid cursor format".to_string()))?;
    let id = parts
        .next()
        .ok_or_else(|| CoreError::InternalError("Invalid cursor format".to_string()))?;

    let parsed_at = DateTime::parse_from_rfc3339(timestamp)
        .map_err(|e| CoreError::InternalError(format!("Invalid cursor timestamp: {}", e)))?
        .with_timezone(&Utc);

    let parsed_id = Uuid::parse_str(id)
        .map_err(|e| CoreError::InternalError(format!("Invalid cursor id: {}", e)))?;

    Ok((parsed_at, parsed_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use uuid::Uuid;

    fn sample_time() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn sample_action_with_target(kind: TargetKind) -> Action {
        Action {
            id: ActionId(Uuid::new_v4()),
            action_type: ActionType("deployment.create".to_string()),
            target: ActionTarget {
                kind,
                id: Uuid::new_v4(),
            },
            payload: ActionPayload { data: Value::Null },
            version: ActionVersion(1),
            status: ActionStatus::Pending,
            metadata: ActionMetadata {
                source: ActionSource::System,
                created_at: sample_time(),
                constraints: ActionConstraints {
                    not_after: None,
                    priority: None,
                },
            },
        }
    }

    #[test]
    fn deployment_id_from_action_accepts_deployment_target() {
        let action = sample_action_with_target(TargetKind::Deployment);
        let expected = DeploymentId(action.target.id);

        let deployment_id = deployment_id_from_action(&action).unwrap();

        assert_eq!(deployment_id.0, expected.0);
    }

    #[test]
    fn deployment_id_from_action_rejects_non_deployment_target() {
        let action = sample_action_with_target(TargetKind::Realm);

        let err = deployment_id_from_action(&action).unwrap_err();

        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("Action target must be a deployment"))
        );
    }

    #[test]
    fn status_to_row_maps_variants() {
        let at = sample_time();
        let pulled = ActionStatus::Pulled {
            agent_id: "agent-1".to_string(),
            at,
        };
        let published = ActionStatus::Published { at };
        let failed = ActionStatus::Failed {
            reason: ActionFailureReason::Timeout,
            at,
        };

        assert_eq!(
            status_to_row(&ActionStatus::Pending),
            ("pending".to_string(), None, None, None)
        );
        assert_eq!(
            status_to_row(&pulled),
            (
                "pulled".to_string(),
                Some(at),
                Some("agent-1".to_string()),
                None
            )
        );
        assert_eq!(
            status_to_row(&published),
            ("published".to_string(), Some(at), None, None)
        );
        assert_eq!(
            status_to_row(&failed),
            (
                "failed".to_string(),
                Some(at),
                None,
                Some("timeout".to_string())
            )
        );
    }

    #[test]
    fn parse_status_handles_success_cases() {
        let at = sample_time();
        let pulled = parse_status("pulled", Some(at), Some("agent-9"), None).unwrap();
        let published = parse_status("published", Some(at), None, None).unwrap();
        let failed = parse_status("failed", Some(at), None, Some("publish_failed")).unwrap();

        assert_eq!(
            pulled,
            ActionStatus::Pulled {
                agent_id: "agent-9".to_string(),
                at
            }
        );
        assert_eq!(published, ActionStatus::Published { at });
        assert_eq!(
            failed,
            ActionStatus::Failed {
                reason: ActionFailureReason::PublishFailed,
                at
            }
        );
    }

    #[test]
    fn parse_status_reports_missing_fields() {
        let at = sample_time();

        let err = parse_status("pulled", Some(at), None, None).unwrap_err();
        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("Missing status_agent_id"))
        );

        let err = parse_status("published", None, None, None).unwrap_err();
        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("Missing status_at"))
        );

        let err = parse_status("failed", Some(at), None, None).unwrap_err();
        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("Missing status_reason"))
        );
    }

    #[test]
    fn parse_status_rejects_unknown_values() {
        let err = parse_status("mystery", None, None, None).unwrap_err();
        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("Unknown action status"))
        );
    }

    #[test]
    fn failure_reason_round_trip() {
        let cases = [
            ActionFailureReason::InvalidPayload,
            ActionFailureReason::UnsupportedAction,
            ActionFailureReason::PublishFailed,
            ActionFailureReason::Timeout,
            ActionFailureReason::InternalError("boom".to_string()),
        ];

        for reason in cases {
            let raw = failure_reason_to_string(&reason);
            let parsed = parse_failure_reason(&raw);
            assert_eq!(parsed, reason);
        }

        let parsed = parse_failure_reason("weird");
        assert_eq!(
            parsed,
            ActionFailureReason::InternalError("weird".to_string())
        );
    }

    #[test]
    fn source_round_trip() {
        let user_uuid = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
        let (user_type, user_id, user_client) =
            source_to_row(&ActionSource::User { user_id: user_uuid });
        assert_eq!(user_type, "user");
        assert_eq!(user_id, Some(user_uuid));
        assert!(user_client.is_none());
        assert_eq!(
            parse_source("USER", user_id, None).unwrap(),
            ActionSource::User { user_id: user_uuid }
        );

        let (system_type, system_user, system_client) = source_to_row(&ActionSource::System);
        assert_eq!(system_type, "system");
        assert!(system_user.is_none());
        assert!(system_client.is_none());
        assert_eq!(
            parse_source("system", None, None).unwrap(),
            ActionSource::System
        );

        let (api_type, api_user, api_client) = source_to_row(&ActionSource::Api {
            client_id: "client-1".to_string(),
        });
        assert_eq!(api_type, "api");
        assert!(api_user.is_none());
        assert_eq!(api_client.as_deref(), Some("client-1"));
        assert_eq!(
            parse_source("api", None, Some("client-1")).unwrap(),
            ActionSource::Api {
                client_id: "client-1".to_string()
            }
        );
    }

    #[test]
    fn parse_source_reports_missing_fields() {
        let err = parse_source("user", None, None).unwrap_err();
        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("Missing source_user_id"))
        );

        let err = parse_source("api", None, None).unwrap_err();
        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("Missing source_client_id"))
        );
    }

    #[test]
    fn target_kind_round_trip() {
        assert_eq!(target_kind_to_string(&TargetKind::Deployment), "deployment");
        assert_eq!(parse_target_kind("REALM"), TargetKind::Realm);
        assert_eq!(
            parse_target_kind("CustomThing"),
            TargetKind::Custom("customthing".to_string())
        );
    }

    #[test]
    fn parse_cursor_handles_valid_and_invalid() {
        let cursor = ActionCursor::new("2025-01-01T00:00:00Z|aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let (timestamp, id) = parse_cursor(&cursor).unwrap();

        assert_eq!(timestamp, sample_time());
        assert_eq!(
            id,
            Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap()
        );

        let err = parse_cursor(&ActionCursor::new("bad")).unwrap_err();
        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("Invalid cursor format"))
        );

        let err = parse_cursor(&ActionCursor::new(
            "not-a-time|aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        ))
        .unwrap_err();
        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("Invalid cursor timestamp"))
        );

        let err = parse_cursor(&ActionCursor::new("2025-01-01T00:00:00Z|not-a-uuid")).unwrap_err();
        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("Invalid cursor id"))
        );
    }
}
