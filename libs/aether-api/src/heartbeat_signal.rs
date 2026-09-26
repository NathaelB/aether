use std::time::Duration;

use aether_core::dataplane::ports::DataPlaneService;
use aether_core::dataplane::value_objects::DataPlaneLiveness;
use aether_core::signals::{Signal, SignalId, SignalKind, SignalSubject};
use chrono::Utc;
use tokio::time::interval;
use tracing::{error, info};
use uuid::Uuid;

use crate::state::AppState;

const EVERY: Duration = Duration::from_secs(30);

pub async fn run_heartbeat_signal_probe(state: AppState) {
    info!("starting heartbeat signal probe");

    let mut ticker = interval(EVERY);

    loop {
        ticker.tick().await;

        let window = state.service.heartbeat_window();
        let now = Utc::now();

        let dataplanes = match state.service.list_all_dataplanes().await {
            Ok(dataplanes) => dataplanes,
            Err(err) => {
                error!(%err, "failed to list data planes for heartbeat signal probe");
                continue;
            }
        };

        for dataplane in dataplanes {
            let liveness = dataplane.liveness(now, window);

            match liveness {
                DataPlaneLiveness::Unreachable => {
                    let dedup_key = format!("dataplane-heartbeat-stale-{}", dataplane.id.0);

                    let last_seen_duration = dataplane
                        .last_seen_at
                        .map(|lst| now - lst)
                        .unwrap_or_default();

                    let message = format!(
                        "Data plane {} has not reported a heartbeat for {} seconds",
                        dataplane.id.0,
                        last_seen_duration.num_seconds()
                    );

                    let signal = Signal::open(
                        SignalId(Uuid::new_v4()),
                        SignalKind::DataplaneHeartbeatStale,
                        SignalSubject::Dataplane { id: dataplane.id },
                        dedup_key,
                        message,
                        now,
                    );

                    if let Err(err) = state.service.write_signal(signal).await {
                        error!(
                            dataplane_id = %dataplane.id.0,
                            %err,
                            "failed to write heartbeat stale signal"
                        );
                    }
                }
                DataPlaneLiveness::Reachable | DataPlaneLiveness::NeverSeen => {
                    let dedup_key = format!("dataplane-heartbeat-stale-{}", dataplane.id.0);

                    if let Err(err) = state.service.close_signal(&dedup_key, now).await {
                        error!(
                            dataplane_id = %dataplane.id.0,
                            %err,
                            "failed to close heartbeat stale signal"
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recently_reporting_dataplane_is_reachable() {
        use aether_core::dataplane::entities::DataPlane;
        use aether_core::dataplane::value_objects::{
            Capacity, DataPlaneAllocation, DataPlaneId, DataPlaneStatus, Region,
        };

        let now = Utc::now();
        let dp = DataPlane {
            id: DataPlaneId(Uuid::new_v4()),
            allocation: DataPlaneAllocation::Shared,
            region: Region::new("fr-par"),
            status: DataPlaneStatus::Active,
            capacity: Capacity::new(5000, 10240, 10).expect("non-zero capacity"),
            last_seen_at: Some(now - chrono::Duration::seconds(30)),
            created_at: now - chrono::Duration::minutes(10),
            operator_version: None,
            gateway_address: None,
            herald: None,
        };

        let window = chrono::Duration::seconds(90);
        assert_eq!(dp.liveness(now, window), DataPlaneLiveness::Reachable);
    }

    #[test]
    fn a_silent_dataplane_is_unreachable() {
        use aether_core::dataplane::entities::DataPlane;
        use aether_core::dataplane::value_objects::{
            Capacity, DataPlaneAllocation, DataPlaneId, DataPlaneStatus, Region,
        };

        let now = Utc::now();
        let dp = DataPlane {
            id: DataPlaneId(Uuid::new_v4()),
            allocation: DataPlaneAllocation::Shared,
            region: Region::new("fr-par"),
            status: DataPlaneStatus::Active,
            capacity: Capacity::new(5000, 10240, 10).expect("non-zero capacity"),
            last_seen_at: Some(now - chrono::Duration::seconds(120)),
            created_at: now - chrono::Duration::minutes(10),
            operator_version: None,
            gateway_address: None,
            herald: None,
        };

        let window = chrono::Duration::seconds(90);
        assert_eq!(dp.liveness(now, window), DataPlaneLiveness::Unreachable);
    }

    #[test]
    fn a_dataplane_that_never_reported_is_never_seen() {
        use aether_core::dataplane::entities::DataPlane;
        use aether_core::dataplane::value_objects::{
            Capacity, DataPlaneAllocation, DataPlaneId, DataPlaneStatus, Region,
        };

        let now = Utc::now();
        let dp = DataPlane {
            id: DataPlaneId(Uuid::new_v4()),
            allocation: DataPlaneAllocation::Shared,
            region: Region::new("fr-par"),
            status: DataPlaneStatus::Provisioning,
            capacity: Capacity::new(5000, 10240, 10).expect("non-zero capacity"),
            last_seen_at: None,
            created_at: now - chrono::Duration::minutes(5),
            operator_version: None,
            gateway_address: None,
            herald: None,
        };

        let window = chrono::Duration::seconds(90);
        assert_eq!(dp.liveness(now, window), DataPlaneLiveness::NeverSeen);
    }

    #[test]
    fn dedup_key_format_is_stable() {
        let id = Uuid::nil();
        let dedup_key = format!("dataplane-heartbeat-stale-{}", id);

        assert_eq!(
            dedup_key,
            "dataplane-heartbeat-stale-00000000-0000-0000-0000-000000000000"
        );
    }
}
