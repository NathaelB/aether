use chrono::{DateTime, Utc};

use aether_domain::{
    CoreError,
    deployments::{
        DeploymentId,
        reachability_history::{
            DeploymentUptime, ReachabilityCheck, ReachabilityCheckRepository, UptimeWindow,
        },
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

    async fn get_uptime(&self, deployment_id: DeploymentId) -> Result<DeploymentUptime, CoreError> {
        let mut tx = self.tx.lock().await;

        let now = Utc::now();

        // Query for 24-hour window
        let checks_24h: Vec<(bool,)> = sqlx::query_as(
            r#"
            SELECT reachable
            FROM deployment_reachability_checks
            WHERE deployment_id = $1
              AND checked_at > $2 - INTERVAL '24 hours'
            ORDER BY checked_at DESC
            "#,
        )
        .bind(deployment_id.0)
        .bind(now)
        .fetch_all(&mut ***tx)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to query 24h checks: {e}"),
        })?;

        // Query for 7-day window
        let checks_7d: Vec<(bool,)> = sqlx::query_as(
            r#"
            SELECT reachable
            FROM deployment_reachability_checks
            WHERE deployment_id = $1
              AND checked_at > $2 - INTERVAL '7 days'
            ORDER BY checked_at DESC
            "#,
        )
        .bind(deployment_id.0)
        .bind(now)
        .fetch_all(&mut ***tx)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to query 7d checks: {e}"),
        })?;

        // Query for 30-day window
        let checks_30d: Vec<(bool,)> = sqlx::query_as(
            r#"
            SELECT reachable
            FROM deployment_reachability_checks
            WHERE deployment_id = $1
              AND checked_at > $2 - INTERVAL '30 days'
            ORDER BY checked_at DESC
            "#,
        )
        .bind(deployment_id.0)
        .bind(now)
        .fetch_all(&mut ***tx)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to query 30d checks: {e}"),
        })?;

        // Also get the earliest check time to determine if windows cover the full period
        let earliest_check: Option<DateTime<Utc>> = sqlx::query_scalar(
            r#"
            SELECT MIN(checked_at)
            FROM deployment_reachability_checks
            WHERE deployment_id = $1
            "#,
        )
        .bind(deployment_id.0)
        .fetch_optional(&mut ***tx)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to query earliest check: {e}"),
        })?
        .flatten();

        let uptime_24h = compute_uptime_window(
            &checks_24h,
            now,
            chrono::Duration::hours(24),
            earliest_check,
        );
        let uptime_7d =
            compute_uptime_window(&checks_7d, now, chrono::Duration::days(7), earliest_check);
        let uptime_30d =
            compute_uptime_window(&checks_30d, now, chrono::Duration::days(30), earliest_check);

        Ok(DeploymentUptime {
            deployment_id,
            uptime_24h,
            uptime_7d,
            uptime_30d,
        })
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

/// Compute uptime percentage for a window of check results.
///
/// If checks_vec is empty or the window is shorter than window_duration,
/// covers_full_window is set to false.
fn compute_uptime_window(
    checks_vec: &[(bool,)],
    now: DateTime<Utc>,
    window_duration: chrono::Duration,
    earliest_check: Option<DateTime<Utc>>,
) -> UptimeWindow {
    if checks_vec.is_empty() {
        return UptimeWindow {
            uptime_percent: 100.0,
            covers_full_window: false,
        };
    }

    let passed = checks_vec.iter().filter(|(r,)| *r).count() as f64;
    let total = checks_vec.len() as f64;
    let uptime_percent = if total > 0.0 {
        (passed / total) * 100.0
    } else {
        100.0
    };

    // Check if window is full or partial
    let window_start = now - window_duration;
    let covers_full = earliest_check
        .map(|earliest| earliest <= window_start)
        .unwrap_or(false);

    UptimeWindow {
        uptime_percent,
        covers_full_window: covers_full,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_uptime_100_percent() {
        let checks = vec![(true,), (true,), (true,)];
        let now = Utc::now();
        let earliest = Some(now - chrono::Duration::days(30));

        let result = compute_uptime_window(&checks, now, chrono::Duration::hours(24), earliest);

        assert_eq!(result.uptime_percent, 100.0);
        assert!(result.covers_full_window);
    }

    #[test]
    fn compute_uptime_50_percent() {
        let checks = vec![(true,), (false,), (true,), (false,)];
        let now = Utc::now();
        let earliest = Some(now - chrono::Duration::days(30));

        let result = compute_uptime_window(&checks, now, chrono::Duration::hours(24), earliest);

        assert_eq!(result.uptime_percent, 50.0);
        assert!(result.covers_full_window);
    }

    #[test]
    fn compute_uptime_partial_window() {
        let checks = vec![(true,), (true,)];
        let now = Utc::now();
        // Earliest check is only 1 hour ago, but we're asking for 24h window
        let earliest = Some(now - chrono::Duration::hours(1));

        let result = compute_uptime_window(&checks, now, chrono::Duration::hours(24), earliest);

        assert_eq!(result.uptime_percent, 100.0);
        assert!(!result.covers_full_window);
    }

    #[test]
    fn compute_uptime_no_checks() {
        let checks: Vec<(bool,)> = vec![];
        let now = Utc::now();

        let result = compute_uptime_window(&checks, now, chrono::Duration::hours(24), None);

        assert_eq!(result.uptime_percent, 100.0);
        assert!(!result.covers_full_window);
    }
}
