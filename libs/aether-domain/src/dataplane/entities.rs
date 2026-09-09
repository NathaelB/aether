use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use chrono::{DateTime, Duration, Utc};

use crate::{
    dataplane::value_objects::{
        Capacity, DataPlaneId, DataPlaneLiveness, DataPlaneMode, DataPlaneStatus, Region,
    },
    generate_uuid_v7,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DataPlane {
    pub id: DataPlaneId,
    pub mode: DataPlaneMode,
    pub region: Region,
    pub status: DataPlaneStatus,
    pub capacity: Capacity,
    /// When this data plane's Herald last reported. `None` means it has never
    /// reported since registration, which is not the same failure as having
    /// reported and gone quiet.
    pub last_seen_at: Option<DateTime<Utc>>,
}

impl DataPlane {
    pub fn new(mode: DataPlaneMode, region: Region, capacity: Capacity) -> Self {
        Self {
            id: DataPlaneId(generate_uuid_v7()),
            status: DataPlaneStatus::Active,
            capacity,
            mode,
            region,
            last_seen_at: None,
        }
    }

    /// Whether this data plane has reported recently enough to be trusted.
    ///
    /// Derived rather than stored: recovery then costs nothing, since there is
    /// no state to flip back when a data plane starts answering again. It also
    /// keeps `status` meaning what an operator decided, which is a different
    /// question from whether the cluster is answering.
    pub fn liveness(&self, now: DateTime<Utc>, window: Duration) -> DataPlaneLiveness {
        match self.last_seen_at {
            None => DataPlaneLiveness::NeverSeen,
            Some(last_seen_at) if now - last_seen_at <= window => DataPlaneLiveness::Reachable,
            Some(_) => DataPlaneLiveness::Unreachable,
        }
    }

    /// Placement requires both: an operator who left it enabled, and a cluster
    /// that is answering.
    pub fn accepts_placement(&self, now: DateTime<Utc>, window: Duration) -> bool {
        self.status == DataPlaneStatus::Active
            && self.liveness(now, window) == DataPlaneLiveness::Reachable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataplane::value_objects::Capacity;

    /// Time is a parameter, not a side effect: every case below is a pure
    /// comparison against a fixed instant, so nothing here sleeps and nothing
    /// is flaky.
    fn dataplane(last_seen_at: Option<DateTime<Utc>>, status: DataPlaneStatus) -> DataPlane {
        DataPlane {
            id: crate::dataplane::value_objects::DataPlaneId(crate::generate_uuid_v7()),
            mode: DataPlaneMode::Shared,
            region: Region::new("fr-par"),
            status,
            capacity: Capacity::new(5000, 10240, 10).expect("non-zero capacity"),
            last_seen_at,
        }
    }

    fn window() -> Duration {
        Duration::seconds(90)
    }

    #[test]
    fn a_data_plane_that_reported_inside_the_window_is_reachable() {
        let now = Utc::now();
        let dp = dataplane(Some(now - Duration::seconds(30)), DataPlaneStatus::Active);

        assert_eq!(dp.liveness(now, window()), DataPlaneLiveness::Reachable);
        assert!(dp.accepts_placement(now, window()));
    }

    #[test]
    fn a_data_plane_that_went_quiet_is_unreachable() {
        let now = Utc::now();
        let dp = dataplane(Some(now - Duration::seconds(120)), DataPlaneStatus::Active);

        assert_eq!(dp.liveness(now, window()), DataPlaneLiveness::Unreachable);
        assert!(!dp.accepts_placement(now, window()));
    }

    /// The boundary is inclusive: a report landing exactly on the window still
    /// counts. Herald's cycle and the control plane's clock will not agree to
    /// the millisecond, and an exclusive bound would drop a data plane for the
    /// difference.
    #[test]
    fn the_window_boundary_still_counts_as_reachable() {
        let now = Utc::now();
        let dp = dataplane(Some(now - window()), DataPlaneStatus::Active);

        assert_eq!(dp.liveness(now, window()), DataPlaneLiveness::Reachable);
    }

    /// Never having reported is a different situation from having stopped: one
    /// is a data plane still being set up, the other is one that broke.
    #[test]
    fn a_data_plane_that_never_reported_is_not_merely_unreachable() {
        let now = Utc::now();
        let dp = dataplane(None, DataPlaneStatus::Active);

        assert_eq!(dp.liveness(now, window()), DataPlaneLiveness::NeverSeen);
        assert!(!dp.accepts_placement(now, window()));
    }

    /// Recovery costs nothing because liveness is derived. The same value
    /// object answers differently once a newer report exists -- there is no
    /// state to flip back, and therefore no way to forget to flip it.
    #[test]
    fn reporting_again_restores_placement_with_no_state_change() {
        let now = Utc::now();
        let mut dp = dataplane(Some(now - Duration::seconds(600)), DataPlaneStatus::Active);
        assert!(!dp.accepts_placement(now, window()));

        dp.last_seen_at = Some(now);

        assert!(dp.accepts_placement(now, window()));
        assert_eq!(dp.status, DataPlaneStatus::Active, "status is untouched");
    }

    /// Liveness and operator intent are independent, which is why they are
    /// separate types: a reachable data plane an operator drained must still
    /// be excluded.
    #[test]
    fn a_reachable_data_plane_that_was_drained_is_still_excluded() {
        let now = Utc::now();
        for status in [DataPlaneStatus::Draining, DataPlaneStatus::Disabled] {
            let dp = dataplane(Some(now), status);

            assert_eq!(
                dp.liveness(now, window()),
                DataPlaneLiveness::Reachable,
                "{status:?} says nothing about whether the cluster answers"
            );
            assert!(!dp.accepts_placement(now, window()), "{status:?}");
        }
    }

    #[test]
    fn a_new_data_plane_has_never_reported() {
        let dp = DataPlane::new(
            DataPlaneMode::Shared,
            Region::new("fr-par"),
            Capacity::new(5000, 10240, 10).expect("non-zero capacity"),
        );

        assert_eq!(dp.last_seen_at, None);
        assert!(!dp.accepts_placement(Utc::now(), window()));
    }
}
