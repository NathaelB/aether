use std::time::Duration;

use aether_core::signals::{Signal, SignalId, SignalKind, SignalSubject};
use chrono::Utc;
use tokio::time::interval;
use tracing::{error, info};
use uuid::Uuid;

use crate::state::AppState;

/// How often this checks which actions are stuck in leased status past their
/// deadline.
///
/// Shorter than typical action SLA: a stuck action should be signaled promptly
/// so operators can intervene if needed.
const EVERY: Duration = Duration::from_secs(30);

/// Creates and updates signals for actions stuck in leased status past their
/// deadline, and closes them once the action is finally acknowledged (published
/// or failed).
///
/// An action is stuck if it was claimed (moved to Leased status) but the data
/// plane failed to acknowledge it (publish or fail) before the lease expired.
/// This probe catches those cases so operators can diagnose delivery problems.
///
/// Runs beside the server, the same as other background probes: nothing a
/// caller does should be the thing that signals a stuck action, and one that
/// went stuck before this process started is caught up by this loop on startup.
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

        // Closing happens in `AetherService::ack_actions` (aether-core), the
        // instant an action is acked -- not here. This loop only ever sees
        // actions still stuck; it has no way to notice the moment one resolves.
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
