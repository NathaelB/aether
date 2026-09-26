//! Periodic probe for backup signal detection.
//!
//! Opens/closes signals for deployments whose backups are enabled but which have
//! gone past their expected backup cadence without a successful archive, or whose
//! last attempt failed.

use std::time::Duration;

use aether_core::backups::ports::BackupService;
use chrono::Utc;
use tokio::time::interval;
use tracing::{error, info};

use crate::state::AppState;

/// How often this checks for backups that have gone missing or failed.
///
/// Longer than heartbeat or reachability probes: a missing backup is less
/// urgent than a silent data plane or unreachable deployment, and backups
/// may take time to complete.
const EVERY: Duration = Duration::from_secs(300); // 5 minutes

/// Runs the backup signal probe.
///
/// Lists all deployments with enabled backup schedules and checks whether
/// they have had successful backups within their expected cadence. Opens
/// or updates signals for those that have gone past their grace period,
/// and closes signals once a fresh successful backup is recorded.
///
/// Runs beside the server like other background probes: nothing a caller
/// does should be the thing that signals a missing backup, and one that
/// went missing before this process started is caught up on startup.
pub async fn run_backup_signal_probe(state: AppState) {
    info!("starting backup signal probe");

    let mut ticker = interval(EVERY);

    loop {
        ticker.tick().await;

        let now = Utc::now();

        match state.service.find_backup_signals(now).await {
            Ok(signal_updates) => {
                for signal_update in signal_updates {
                    // Write or update the signal for missing/failed backups.
                    if let Some(signal) = signal_update.signal_to_open {
                        if let Err(err) = state.service.write_signal(signal).await {
                            error!(
                                deployment_id = %signal_update.deployment_id,
                                %err,
                                "failed to write backup signal"
                            );
                        }
                    } else if signal_update.should_close {
                        // Close any existing signal if the backup is now healthy.
                        if let Err(err) = state
                            .service
                            .close_signal(&signal_update.dedup_key_prefix, now)
                            .await
                        {
                            error!(
                                deployment_id = %signal_update.deployment_id,
                                %err,
                                "failed to close backup signal"
                            );
                        }
                    }
                }
            }
            Err(err) => {
                error!(%err, "failed to find backup signals");
                continue;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    /// Test that the probe interval is reasonable.
    #[test]
    fn probe_interval_is_reasonable() {
        // The probe should run every 5 minutes, which is longer than
        // heartbeat (30s) and reachability (60s) but still responsive to
        // missing backups.
        assert_eq!(EVERY.as_secs(), 300);
    }

    /// Test dedup key format is stable for missing backups.
    #[test]
    fn dedup_key_format_for_missing_is_stable() {
        let id = Uuid::nil();
        let dedup_key = format!("backup-missing-{}", id);

        assert_eq!(
            dedup_key,
            "backup-missing-00000000-0000-0000-0000-000000000000"
        );
    }

    /// Test dedup key format is stable for failed backups.
    #[test]
    fn dedup_key_format_for_failed_is_stable() {
        let id = Uuid::nil();
        let dedup_key = format!("backup-failed-{}", id);

        assert_eq!(
            dedup_key,
            "backup-failed-00000000-0000-0000-0000-000000000000"
        );
    }
}
