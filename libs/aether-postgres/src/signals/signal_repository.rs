use chrono::{DateTime, Utc};
use uuid::Uuid;

use aether_domain::{
    CoreError,
    signals::{Signal, SignalSubject, ports::SignalRepository},
};
use aether_macros::repository;
use aether_persistence::SharedTx;

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
}

fn subject_to_row(subject: &SignalSubject) -> (String, Option<Uuid>, Option<Uuid>, Option<Uuid>) {
    match subject {
        SignalSubject::Dataplane { id } => ("dataplane".to_string(), Some(id.0), None, None),
        SignalSubject::Deployment { id } => ("deployment".to_string(), None, Some(id.0), None),
        SignalSubject::Action { id } => ("action".to_string(), None, None, Some(*id)),
    }
}
