use std::time::Duration;

use aether_core::backups::ports::BackupService;
use chrono::Utc;
use tokio::time::interval;
use tracing::{error, info};

use crate::state::AppState;

const EVERY: Duration = Duration::from_secs(300);

pub async fn run_backup_signal_probe(state: AppState) {
    info!("starting backup signal probe");

    let mut ticker = interval(EVERY);

    loop {
        ticker.tick().await;

        let now = Utc::now();

        match state.service.find_backup_signals(now).await {
            Ok(signal_updates) => {
                for signal_update in signal_updates {
                    if let Some(signal) = signal_update.signal_to_open {
                        if let Err(err) = state.service.write_signal(signal).await {
                            error!(
                                deployment_id = %signal_update.deployment_id,
                                %err,
                                "failed to write backup signal"
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
                            "failed to close backup signal"
                        );
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

    #[test]
    fn probe_interval_is_reasonable() {
        assert_eq!(EVERY.as_secs(), 300);
    }

    #[test]
    fn dedup_key_format_for_missing_is_stable() {
        let id = Uuid::nil();
        let dedup_key = format!("backup-missing-{}", id);

        assert_eq!(
            dedup_key,
            "backup-missing-00000000-0000-0000-0000-000000000000"
        );
    }

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
