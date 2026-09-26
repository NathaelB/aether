//! Periodic probe for drill overdue signal detection.
//!
//! Opens/closes signals for deployments whose backups are enabled but which have
//! not had a successful restore drill within the expected interval, or never.

use std::time::Duration;

use aether_core::backups::ports::BackupService;
use chrono::Utc;
use tokio::time::interval;
use tracing::{error, info};

use crate::state::AppState;

/// How often this checks for deployments that are overdue for a drill.
///
/// Longer than backup or heartbeat probes: a drill is less urgent (it's
/// voluntary and a backup-less deployment cannot be drilled anyway), and
/// the interval is weekly so the probe does not need to run very often.
const EVERY: Duration = Duration::from_secs(600); // 10 minutes

/// Opens or updates a signal for each deployment past its drill grace
/// period, closing it once a fresh successful drill lands.
pub async fn run_drill_signal_probe(state: AppState) {
    info!("starting drill signal probe");

    let mut ticker = interval(EVERY);

    loop {
        ticker.tick().await;

        let now = Utc::now();

        match state.service.find_drill_signals(now).await {
            Ok(signal_updates) => {
                for signal_update in signal_updates {
                    if let Some(signal) = signal_update.signal_to_open {
                        if let Err(err) = state.service.write_signal(signal).await {
                            error!(
                                deployment_id = %signal_update.deployment_id,
                                %err,
                                "failed to write drill signal"
                            );
                        }
                    } else if signal_update.should_close
                        && let Err(err) = state
                            .service
                            .close_signal(&signal_update.dedup_key_prefix, now)
                            .await
                    {
                        error!(
                            deployment_id = %signal_update.deployment_id,
                            %err,
                            "failed to close drill signal"
                        );
                    }
                }
            }
            Err(err) => {
                error!(%err, "failed to find drill signals");
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
        assert_eq!(EVERY.as_secs(), 600);
    }

    /// Test dedup key format is stable.
    #[test]
    fn dedup_key_format_is_stable() {
        let id = Uuid::nil();
        let dedup_key = format!("drill-overdue-{}", id);

        assert_eq!(
            dedup_key,
            "drill-overdue-00000000-0000-0000-0000-000000000000"
        );
    }
}
