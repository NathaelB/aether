use std::time::Duration;

use aether_core::signals::{Signal, SignalId, SignalKind, SignalSubject};
use chrono::Utc;
use tokio::time::interval;
use tracing::{error, info};
use uuid::Uuid;

use crate::state::AppState;

const EVERY: Duration = Duration::from_secs(30);

pub async fn run_action_stuck_signal_probe(state: AppState) {
    info!("starting action stuck signal probe");

    let mut ticker = interval(EVERY);

    loop {
        ticker.tick().await;

        let stuck_actions = match state.service.list_stuck_actions().await {
            Ok(actions) => actions,
            Err(err) => {
                error!(%err, "failed to list stuck actions for probe");
                continue;
            }
        };

        let now = Utc::now();

        for action in stuck_actions {
            let dedup_key = format!("action-stuck-{}", action.id.0);

            let message = format!(
                "Action {} in deployment {} is stuck in leased status on data plane {}",
                action.id.0, action.deployment_id.0, action.dataplane_id.0
            );

            let signal = Signal::open(
                SignalId(Uuid::new_v4()),
                SignalKind::ActionStuck,
                SignalSubject::Action { id: action.id.0 },
                dedup_key,
                message,
                now,
            );

            if let Err(err) = state.service.write_signal(signal).await {
                error!(
                    action_id = %action.id.0,
                    %err,
                    "failed to write action stuck signal"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_key_format_is_stable() {
        let id = Uuid::nil();
        let dedup_key = format!("action-stuck-{}", id);

        assert_eq!(
            dedup_key,
            "action-stuck-00000000-0000-0000-0000-000000000000"
        );
    }
}
