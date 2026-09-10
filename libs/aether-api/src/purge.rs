use std::time::Duration;

use aether_core::deployments::ports::DeploymentService;
use tokio::time::interval;
use tracing::{error, info};

use crate::state::AppState;

/// How often the purge runs. The retention window is measured in days, so the
/// exact moment a row goes does not matter -- only that it eventually does.
const EVERY: Duration = Duration::from_secs(60 * 60);

/// Removes deployments whose tear-down was confirmed longer ago than the
/// retention window.
///
/// Runs beside the server rather than on a request: nothing a caller does
/// should be the thing that finally clears rows, and a deployment removed
/// months ago has no request to hang off.
pub async fn purge_deleted_deployments(state: AppState) {
    let retention = state.service.deleted_retention();

    if retention <= chrono::Duration::zero() {
        info!("deleted deployments are kept indefinitely");
        return;
    }

    info!(days = retention.num_days(), "purging deleted deployments");

    let mut ticker = interval(EVERY);

    loop {
        ticker.tick().await;

        match state.service.purge_deleted_deployments(retention).await {
            Ok(0) => {}
            Ok(purged) => info!(purged, "removed deployments past their retention"),
            Err(err) => error!(%err, "failed to purge deleted deployments"),
        }
    }
}
