use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use aether_domain::{
    CoreError,
    signals::{
        Signal, SignalId, SignalKind, SignalSubject,
        ports::{SignalListPage, SignalRepository},
    },
};
use aether_macros::repository;
use aether_persistence::SharedTx;

#[derive(Clone, FromRow)]
struct SignalRow {
    id: Uuid,
    kind: String,
    subject_kind: String,
    subject_dataplane_id: Option<Uuid>,
    subject_deployment_id: Option<Uuid>,
    subject_action_id: Option<Uuid>,
    dedup_key: String,
    message: String,
    opened_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
}

impl SignalRow {
    fn into_signal(self) -> Result<Signal, CoreError> {
        let kind = self.kind.parse::<SignalKind>()?;

        let subject = match self.subject_kind.as_str() {
            "dataplane" => SignalSubject::Dataplane {
                id: aether_domain::dataplane::value_objects::DataPlaneId(
                    self.subject_dataplane_id.ok_or_else(|| {
                        CoreError::InternalError(
                            "Signal with dataplane kind has no dataplane_id".to_string(),
                        )
                    })?,
                ),
            },
            "deployment" => SignalSubject::Deployment {
                id: aether_domain::deployments::DeploymentId(
                    self.subject_deployment_id.ok_or_else(|| {
                        CoreError::InternalError(
                            "Signal with deployment kind has no deployment_id".to_string(),
                        )
                    })?,
                ),
            },
            "action" => SignalSubject::Action {
                id: self.subject_action_id.ok_or_else(|| {
                    CoreError::InternalError("Signal with action kind has no action_id".to_string())
                })?,
            },
            other => {
                return Err(CoreError::InternalError(format!(
                    "Unknown signal subject kind: {other}"
                )));
            }
        };

        Ok(Signal {
            id: SignalId(self.id),
            kind,
            subject,
            dedup_key: self.dedup_key,
            message: self.message,
            opened_at: self.opened_at,
            last_seen_at: self.last_seen_at,
            closed_at: self.closed_at,
        })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[repository(domain = Signal, backend = Postgres)]
pub struct PostgresSignalRepository<'tx> {
    tx: SharedTx<'tx>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'tx> PostgresSignalRepository<'tx> {
    pub fn new(tx: &SharedTx<'tx>) -> Self {
        Self { tx: tx.clone() }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl SignalRepository for PostgresSignalRepository<'_> {
    async fn write(&self, signal: Signal) -> Result<(), CoreError> {
        let (subject_kind, subject_dataplane_id, subject_deployment_id, subject_action_id) =
            subject_to_row(&signal.subject);

        {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            INSERT INTO signals (
                id,
                kind,
                subject_kind,
                subject_dataplane_id,
                subject_deployment_id,
                subject_action_id,
                dedup_key,
                message,
                opened_at,
                last_seen_at,
                closed_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (dedup_key) WHERE closed_at IS NULL
            DO UPDATE SET
                message = $8,
                last_seen_at = $10
            "#,
                signal.id.0,
                signal.kind.to_string(),
                subject_kind,
                subject_dataplane_id,
                subject_deployment_id,
                subject_action_id,
                signal.dedup_key,
                signal.message,
                signal.opened_at,
                signal.last_seen_at,
                signal.closed_at,
            )
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to write signal: {e}"),
        })?;

        Ok(())
    }

    async fn close(&self, dedup_key: &str, at: DateTime<Utc>) -> Result<(), CoreError> {
        {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            UPDATE signals
            SET closed_at = $1
            WHERE dedup_key = $2 AND closed_at IS NULL
            "#,
                at,
                dedup_key,
            )
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to close signal: {e}"),
        })?;

        Ok(())
    }

    async fn list_open(
        &self,
        kind_filter: Option<SignalKind>,
        subject_filter: Option<SignalSubject>,
        limit: usize,
        cursor: Option<String>,
    ) -> Result<SignalListPage, CoreError> {
        let mut tx = self.tx.lock().await;

        let (
            subject_kind_filter,
            subject_dataplane_id_filter,
            subject_deployment_id_filter,
            subject_action_id_filter,
        ) = if let Some(subject) = subject_filter {
            subject_to_row(&subject)
        } else {
            (String::new(), None, None, None)
        };

        let kind_filter_str = kind_filter.map(|k| k.to_string());

        // Cursor carries both `opened_at` and `id`: two signals opened in the
        // same probe tick share the exact same `opened_at` (the probe reads
        // `Utc::now()` once per tick, not once per signal), so `opened_at`
        // alone cannot tell them apart at a page boundary -- rows tied with
        // the boundary row would silently vanish from every later page.
        // Encoded as "<opened_at RFC3339>|<id>"; a value that doesn't parse
        // this way is treated as no cursor rather than an error, matching how
        // this method treats any other malformed filter.
        let cursor_key = cursor.as_ref().and_then(|raw| {
            let (ts, id) = raw.split_once('|')?;
            let ts = ts.parse::<DateTime<Utc>>().ok()?;
            let id = id.parse::<Uuid>().ok()?;
            Some((ts, id))
        });

        // Query with filters. We get limit+1 to detect if there's a next page.
        // Keyset pagination on (opened_at, id) -- see the cursor comment above
        // for why `id` has to be part of the key, not just an ORDER BY nicety.
        let rows: Vec<SignalRow> = {
            sqlx::query_as(
                r#"
                SELECT
                    id,
                    kind,
                    subject_kind,
                    subject_dataplane_id,
                    subject_deployment_id,
                    subject_action_id,
                    dedup_key,
                    message,
                    opened_at,
                    last_seen_at,
                    closed_at
                FROM signals
                WHERE closed_at IS NULL
                    AND ($1::text IS NULL OR kind = $1)
                    AND ($2::text = '' OR subject_kind = $2)
                    AND (COALESCE($3::uuid, subject_dataplane_id) = subject_dataplane_id OR $2::text != 'dataplane')
                    AND (COALESCE($4::uuid, subject_deployment_id) = subject_deployment_id OR $2::text != 'deployment')
                    AND (COALESCE($5::uuid, subject_action_id) = subject_action_id OR $2::text != 'action')
                    AND (
                        $6::timestamptz IS NULL
                        OR (opened_at, id) < ($6::timestamptz, $7::uuid)
                    )
                ORDER BY opened_at DESC, id DESC
                LIMIT $8::bigint
                "#,
            )
            .bind(kind_filter_str.as_deref())
            .bind(if !subject_kind_filter.is_empty() {
                Some(&subject_kind_filter)
            } else {
                None
            })
            .bind(subject_dataplane_id_filter)
            .bind(subject_deployment_id_filter)
            .bind(subject_action_id_filter)
            .bind(cursor_key.map(|(ts, _)| ts))
            .bind(cursor_key.map(|(_, id)| id))
            .bind((limit + 1) as i64)
            .fetch_all(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list signals: {e}"),
        })?;

        // Parse rows into signals and detect if there's a next page.
        let has_next = rows.len() > limit;
        let signals_rows = if has_next { &rows[..limit] } else { &rows[..] };

        let mut signals = Vec::new();
        for row in signals_rows {
            let signal = row.clone().into_signal()?;
            signals.push(signal);
        }

        let next_cursor = if has_next {
            signals_rows
                .last()
                .map(|row| format!("{}|{}", row.opened_at.to_rfc3339(), row.id))
        } else {
            None
        };

        Ok(SignalListPage {
            signals,
            next_cursor,
        })
    }
}

fn subject_to_row(subject: &SignalSubject) -> (String, Option<Uuid>, Option<Uuid>, Option<Uuid>) {
    match subject {
        SignalSubject::Dataplane { id } => ("dataplane".to_string(), Some(id.0), None, None),
        SignalSubject::Deployment { id } => ("deployment".to_string(), None, Some(id.0), None),
        SignalSubject::Action { id } => ("action".to_string(), None, None, Some(*id)),
    }
}
