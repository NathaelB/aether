use std::time::Duration;

use aether_core::deployments::ports::DeploymentService;
use tokio::time::interval;
use tracing::{error, info};

use crate::state::AppState;

/// How often the purge runs. The retention window is measured in days, so the
/// exact moment a row goes does not matter -- only that it eventually does.
const EVERY: Duration = Duration::from_secs(60 * 60);

/// Reachability check retention window (30 days).
const REACHABILITY_CHECK_RETENTION_DAYS: i64 = 30;

/// Removes deployments whose tear-down was confirmed longer ago than the
/// retention window, and purges reachability check history older than the
/// retention window.
///
/// Runs beside the server rather than on a request: nothing a caller does
/// should be the thing that finally clears rows, and a deployment removed
/// months ago has no request to hang off.
pub async fn purge_deleted_deployments(state: AppState) {
    let deployment_retention = state.service.deleted_retention();

    if deployment_retention <= chrono::Duration::zero() {
        info!("deleted deployments are kept indefinitely");
    } else {
        info!(
            days = deployment_retention.num_days(),
            "purging deleted deployments"
        );
    }

    let check_retention = chrono::Duration::days(REACHABILITY_CHECK_RETENTION_DAYS);
    info!(
        days = check_retention.num_days(),
        "purging reachability checks"
    );

    let mut ticker = interval(EVERY);

    loop {
        ticker.tick().await;

        // Purge deleted deployments if retention is configured
        if deployment_retention > chrono::Duration::zero() {
            match state
                .service
                .purge_deleted_deployments(deployment_retention)
                .await
            {
                Ok(0) => {}
                Ok(purged) => info!(purged, "removed deployments past their retention"),
                Err(err) => error!(%err, "failed to purge deleted deployments"),
            }
        }

        // Purge old reachability checks
        match state
            .service
            .purge_old_reachability_checks(check_retention)
            .await
        {
            Ok(0) => {}
            Ok(purged) => info!(purged, "removed reachability checks past their retention"),
            Err(err) => error!(%err, "failed to purge reachability checks"),
        }
    }
}
