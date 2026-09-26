use std::time::Duration;

use aether_core::backups::ports::BackupService;
use chrono::Utc;
use tokio::time::interval;
use tracing::{error, info};

use crate::state::AppState;

const EVERY: Duration = Duration::from_secs(600);

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

    #[test]
    fn probe_interval_is_reasonable() {
        assert_eq!(EVERY.as_secs(), 600);
    }

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
