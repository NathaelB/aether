use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use chrono::{DateTime, Duration, Utc};

use crate::{
    CoreError,
    dataplane::value_objects::{
        Capacity, DataPlaneAllocation, DataPlaneId, DataPlaneLiveness, DataPlaneStatus, Region,
    },
    generate_uuid_v7,
    version::Version,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DataPlane {
    pub id: DataPlaneId,
    pub allocation: DataPlaneAllocation,
    pub region: Region,
    pub status: DataPlaneStatus,
    pub capacity: Capacity,
    /// When this data plane's Herald last reported. `None` means it has never
    /// reported since registration, which is not the same failure as having
    /// reported and gone quiet.
    pub last_seen_at: Option<DateTime<Utc>>,
    /// When this data plane was registered. What `Provisioning` is measured
    /// against: without it, a plane that never came up is indistinguishable
    /// from one created a second ago.
    pub created_at: DateTime<Utc>,
    /// Which identity may speak for this data plane.
    ///
    /// `None` for one registered before clusters had identities of their own.
    /// Such a data plane cannot be spoken for once the shared client is gone,
    /// which is deliberate: the recovery is re-issuing its credential, not
    /// trusting whoever asks.
    pub herald: Option<crate::dataplane::herald_identity::HeraldBinding>,

    /// The operator/chart version this data plane's Herald last reported
    /// running. `None` until the first heartbeat that carries one.
    ///
    /// There is one operator and one chart per cluster, shared by every
    /// tenant on it, so this is a fact about the cluster rather than about any
    /// one deployment -- which is why it lives here and not on `Deployment`.
    pub operator_version: Option<Version>,

    /// Where this data plane's own Gateway answers -- a LoadBalancer address
    /// Kubernetes already assigned it, reported back with every heartbeat.
    /// `None` until the first heartbeat that carries one, same as
    /// `operator_version`.
    ///
    /// What a deployment's own hostname eventually resolves to: this is the
    /// address a DNS record for `<deployment>.autharie.fr` would point at,
    /// not anything the control plane invents.
    pub gateway_address: Option<String>,
}

impl DataPlane {
    /// Starts in `Provisioning`, not `Active`.
    ///
    /// Registering a data plane says it should exist, not that it can serve:
    /// a cluster takes minutes to create, and placing a deployment on one that
    /// is still coming up would strand it.
    pub fn new(allocation: DataPlaneAllocation, region: Region, capacity: Capacity) -> Self {
        Self {
            id: DataPlaneId(generate_uuid_v7()),
            status: DataPlaneStatus::Provisioning,
            capacity,
            allocation,
            region,
            last_seen_at: None,
            created_at: Utc::now(),
            // Minted after this, by whoever registers it: creating a client is
            // a call to another system, and a constructor that can fail on the
            // network is one every test has to hold an opinion about.
            herald: None,
            operator_version: None,
            gateway_address: None,
        }
    }

    /// Whether this data plane was supposed to come up and did not.
    ///
    /// `Provisioning` is trusted on purpose -- it is the state every dedicated
    /// cluster passes through before its Herald reports, and refusing it would
    /// make the first deployment on an organisation's own cluster impossible.
    /// But the trust had no upper bound, so a provision that half-succeeded
    /// kept accepting every deployment that organisation created, each one
    /// waiting on a Herald that was never coming.
    ///
    /// `last_seen_at` is the discriminator: a plane that has reported once is
    /// a real cluster, whatever its status says.
    pub fn is_stuck_provisioning(&self, now: DateTime<Utc>, timeout: Duration) -> bool {
        self.status == DataPlaneStatus::Provisioning
            && self.last_seen_at.is_none()
            && now - self.created_at > timeout
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

    /// Stops new work from being placed here, and touches nothing running.
    ///
    /// The half of the lifecycle a fleet actually uses: a machine being given
    /// up is emptied long before it is deleted, and deleting it is not what
    /// the operator wants to do first.
    ///
    /// Allowed from any state. Taking something out of service only ever
    /// narrows what it can do, so there is no state from which it is unsafe.
    pub fn drain(&mut self) {
        self.status = DataPlaneStatus::Draining;
    }

    /// Takes it out of service entirely.
    ///
    /// The same effect on placement as draining, and a different thing said
    /// to whoever looks: draining is a cluster on its way out, disabled is
    /// one that should not be used right now. Both exist already; neither had
    /// any way to be set.
    pub fn disable(&mut self) {
        self.status = DataPlaneStatus::Disabled;
    }

    /// Puts it back on the path it was on.
    ///
    /// Not "back to active": a plane that never reported has no business
    /// being called active because somebody clicked. It goes back to
    /// `Provisioning` and the heartbeat promotes it, which is the same
    /// sequence a freshly registered plane follows -- and the only sequence
    /// in which `Active` means a cluster answered.
    ///
    /// `Failed` does not come back. It records that provisioning did not
    /// complete, and reviving it silently would hide a half-built cluster
    /// behind a status that says otherwise. The way back is registering one
    /// that works.
    pub fn return_to_service(&mut self) -> Result<(), CoreError> {
        if self.status == DataPlaneStatus::Failed {
            return Err(CoreError::DataPlaneCannotReturnToService {
                id: self.id,
                status: self.status.to_string(),
            });
        }

        self.status = match self.last_seen_at {
            Some(_) => DataPlaneStatus::Active,
            None => DataPlaneStatus::Provisioning,
        };

        Ok(())
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
            herald: None,
            id: crate::dataplane::value_objects::DataPlaneId(crate::generate_uuid_v7()),
            allocation: DataPlaneAllocation::Shared,
            region: Region::new("fr-par"),
            status,
            capacity: Capacity::new(5000, 10240, 10).expect("non-zero capacity"),
            last_seen_at,
            created_at: Utc::now(),
            operator_version: None,
            gateway_address: None,
        }
    }

    /// Draining stops placement. That is the whole of what it does: the
    /// deployments already on the cluster are not this method's business.
    #[test]
    fn a_drained_plane_stops_accepting_placement() {
        let mut plane = dataplane(Some(Utc::now()), DataPlaneStatus::Active);
        assert!(plane.accepts_placement(Utc::now(), window()));

        plane.drain();

        assert_eq!(plane.status, DataPlaneStatus::Draining);
        assert!(!plane.accepts_placement(Utc::now(), window()));
    }

    #[test]
    fn a_disabled_plane_stops_accepting_placement() {
        let mut plane = dataplane(Some(Utc::now()), DataPlaneStatus::Active);
        plane.disable();

        assert_eq!(plane.status, DataPlaneStatus::Disabled);
        assert!(!plane.accepts_placement(Utc::now(), window()));
    }

    /// Taking something out of service only ever narrows what it can do, so
    /// there is no state it is unsafe from -- including one that is still
    /// coming up, which is exactly when an operator changes their mind.
    #[test]
    fn a_plane_can_be_taken_out_of_service_from_any_state() {
        for status in [
            DataPlaneStatus::Provisioning,
            DataPlaneStatus::Active,
            DataPlaneStatus::Draining,
            DataPlaneStatus::Disabled,
            DataPlaneStatus::Failed,
        ] {
            let mut plane = dataplane(None, status);
            plane.disable();
            assert_eq!(plane.status, DataPlaneStatus::Disabled);
        }
    }

    /// Back to active, because this cluster has answered before.
    #[test]
    fn a_plane_that_has_reported_comes_back_active() {
        let mut plane = dataplane(Some(Utc::now()), DataPlaneStatus::Draining);

        plane.return_to_service().expect("it may come back");

        assert_eq!(plane.status, DataPlaneStatus::Active);
        assert!(plane.accepts_placement(Utc::now(), window()));
    }

    /// A plane that never reported has no business being called active
    /// because somebody clicked. It rejoins the path a freshly registered one
    /// follows, and the heartbeat is what promotes it -- which is the only
    /// sequence in which `Active` means a cluster answered.
    #[test]
    fn a_plane_that_never_reported_comes_back_provisioning() {
        let mut plane = dataplane(None, DataPlaneStatus::Disabled);

        plane.return_to_service().expect("it may come back");

        assert_eq!(plane.status, DataPlaneStatus::Provisioning);
        assert!(!plane.accepts_placement(Utc::now(), window()));
    }

    /// Reviving it would hide a half-built cluster behind a status saying
    /// otherwise, and strand every deployment placed on it.
    #[test]
    fn a_failed_plane_does_not_come_back() {
        let mut plane = dataplane(Some(Utc::now()), DataPlaneStatus::Failed);

        let refused = plane.return_to_service().expect_err("it came back");

        assert!(matches!(
            refused,
            CoreError::DataPlaneCannotReturnToService { .. }
        ));
        assert_eq!(plane.status, DataPlaneStatus::Failed, "and it did not move");
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
            DataPlaneAllocation::Shared,
            Region::new("fr-par"),
            Capacity::new(5000, 10240, 10).expect("non-zero capacity"),
        );

        assert_eq!(dp.last_seen_at, None);
        assert!(!dp.accepts_placement(Utc::now(), window()));
        assert_eq!(
            dp.status,
            DataPlaneStatus::Provisioning,
            "registering a data plane says it should exist, not that it can serve"
        );
    }

    /// The regression #78 describes: a dedicated plane that entered
    /// `Provisioning` and never came up kept accepting every deployment its
    /// organisation created, each one waiting on a Herald that never existed.
    #[test]
    fn a_plane_that_never_came_up_stops_being_believed() {
        let now = Utc::now();
        let mut plane = dataplane(None, DataPlaneStatus::Provisioning);
        plane.created_at = now - Duration::minutes(31);

        assert!(plane.is_stuck_provisioning(now, Duration::minutes(30)));
    }

    /// The reason the bound exists rather than a blanket refusal: this is the
    /// state every dedicated cluster passes through, and refusing it would
    /// make the first deployment on an organisation's own cluster impossible.
    #[test]
    fn a_plane_that_is_still_coming_up_is_left_alone() {
        let now = Utc::now();
        let mut plane = dataplane(None, DataPlaneStatus::Provisioning);
        plane.created_at = now - Duration::minutes(5);

        assert!(!plane.is_stuck_provisioning(now, Duration::minutes(30)));
    }

    /// Having reported once is stronger evidence than any elapsed time: that
    /// is a real cluster, whatever its status column says.
    #[test]
    fn a_plane_that_has_reported_is_never_stuck() {
        let now = Utc::now();
        let mut plane = dataplane(
            Some(now - Duration::minutes(1)),
            DataPlaneStatus::Provisioning,
        );
        plane.created_at = now - Duration::hours(4);

        assert!(!plane.is_stuck_provisioning(now, Duration::minutes(30)));
    }

    /// The bound is about provisioning only. An old `Active` plane that has
    /// gone quiet is the heartbeat window's business, and answering both
    /// questions here would make one of them unfixable without the other.
    #[test]
    fn only_provisioning_can_be_stuck() {
        let now = Utc::now();

        for status in [
            DataPlaneStatus::Active,
            DataPlaneStatus::Draining,
            DataPlaneStatus::Disabled,
            DataPlaneStatus::Failed,
        ] {
            let mut plane = dataplane(None, status);
            plane.created_at = now - Duration::days(7);

            assert!(
                !plane.is_stuck_provisioning(now, Duration::minutes(30)),
                "{status:?} is not a provisioning failure"
            );
        }
    }
}
