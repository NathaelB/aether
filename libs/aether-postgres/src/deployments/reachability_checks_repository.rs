use chrono::{DateTime, Utc};

use aether_domain::{
    CoreError,
    deployments::{
        DeploymentId,
        reachability_history::{ReachabilityCheck, ReachabilityCheckRepository},
    },
};
use aether_macros::repository;
use aether_persistence::SharedTx;

#[cfg_attr(coverage_nightly, coverage(off))]
#[repository(domain = ReachabilityCheck, backend = Postgres)]
pub struct PostgresReachabilityChecksRepository<'tx> {
    tx: SharedTx<'tx>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'tx> PostgresReachabilityChecksRepository<'tx> {
    pub fn new(tx: &SharedTx<'tx>) -> Self {
        Self { tx: tx.clone() }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ReachabilityCheckRepository for PostgresReachabilityChecksRepository<'_> {
    async fn record_check(&self, check: ReachabilityCheck) -> Result<(), CoreError> {
        {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
                INSERT INTO deployment_reachability_checks (
                    deployment_id,
                    checked_at,
                    reachable
                )
                VALUES ($1, $2, $3)
                "#,
                check.deployment_id.0,
                check.checked_at,
                check.reachable,
            )
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to record reachability check: {e}"),
        })?;

        Ok(())
    }

    async fn get_checks_since(
        &self,
        deployment_id: DeploymentId,
        since: DateTime<Utc>,
    ) -> Result<Vec<ReachabilityCheck>, CoreError> {
        let mut tx = self.tx.lock().await;

        let rows: Vec<(DateTime<Utc>, bool)> = sqlx::query_as(
            r#"
            SELECT checked_at, reachable
            FROM deployment_reachability_checks
            WHERE deployment_id = $1
              AND checked_at >= $2
            ORDER BY checked_at ASC, id ASC
            "#,
        )
        .bind(deployment_id.0)
        .bind(since)
        .fetch_all(&mut ***tx)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to query checks since {since}: {e}"),
        })?;

        Ok(rows
            .into_iter()
            .map(|(checked_at, reachable)| ReachabilityCheck {
                deployment_id,
                checked_at,
                reachable,
            })
            .collect())
    }

    async fn purge_old_checks(&self, retention: chrono::Duration) -> Result<u64, CoreError> {
        let cutoff = Utc::now() - retention;

        let result = {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
                DELETE FROM deployment_reachability_checks
                WHERE checked_at < $1
                "#,
                cutoff,
            )
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to purge old checks: {e}"),
        })?;

        Ok(result.rows_affected())
    }
}
