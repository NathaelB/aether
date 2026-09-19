use std::time::Duration;

use aether_core::backups::ports::BackupService;
use tokio::time::interval;
use tracing::{error, info};

use crate::state::AppState;

/// How often this checks for a deployment whose window just opened.
///
/// Shorter than the purge loop's hourly tick on purpose: a maintenance window
/// can be as short as a few minutes, and a scheduler that only looks once an
/// hour would miss most of them closing again before it ever checked.
const EVERY: Duration = Duration::from_secs(5 * 60);

/// Triggers a restore drill (#185) for every deployment whose maintenance
/// window is open right now and that has gone a week or more without one
/// succeeding.
///
/// Runs beside the server, the same as
/// [`purge_deleted_deployments`](crate::purge::purge_deleted_deployments):
/// nothing a caller does should be the thing that finally proves a restore
/// path works, and a deployment nobody is looking at has no request to hang
/// this off.
pub async fn run_restore_drills(state: AppState) {
    let mut ticker = interval(EVERY);

    loop {
        ticker.tick().await;

        let due = match state.service.deployments_due_for_drill().await {
            Ok(due) => due,
            Err(err) => {
                error!(%err, "failed to read which deployments are due for a drill");
                continue;
            }
        };

        for deployment_id in due {
            match state.service.trigger_drill(deployment_id).await {
                Ok(()) => info!(deployment = %deployment_id.0, "restore drill triggered"),
                Err(err) => error!(
                    %err,
                    deployment = %deployment_id.0,
                    "failed to trigger a restore drill"
                ),
            }
        }
    }
}
