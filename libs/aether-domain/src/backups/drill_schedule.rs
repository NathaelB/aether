//! Deciding, at one moment, which deployments a drill (#185) may run against
//! unattended.
//!
//! Pure, for the same reason [`crate::upgrades::scheduler`] is: a scheduler
//! whose tests sleep is a scheduler nobody runs, and this one spends real
//! capacity on a real data plane without asking first.

use chrono::{DateTime, Duration, Utc};

use crate::deployments::Deployment;

/// How long a deployment goes between drills once one has succeeded.
///
/// Weekly: often enough that a restore path left broken for a quarter is
/// caught well before anyone needs it, and rare enough that the capacity it
/// borrows is a rounding error next to what the deployment runs on every day.
pub const DRILL_INTERVAL: Duration = Duration::weeks(1);

/// Why a deployment was passed over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrillSkipped {
    /// No window, so no unattended action -- the same rule an upgrade is
    /// held to, for the same reason: the platform does not spend a
    /// deployment's own capacity without a window it was given for exactly
    /// that.
    NoWindow,
    /// The window is shut at this moment.
    OutsideWindow,
    /// Drilled inside [`DRILL_INTERVAL`] already; nothing new to prove yet.
    RecentlyVerified,
}

/// Whether `deployment` may be drilled unattended, at `at`.
///
/// Takes the deployment alone rather than an estate: unlike
/// [`crate::upgrades::scheduler::due`], nothing about one deployment's drill
/// depends on another's, so there is no batch decision to make -- only this
/// one, asked once per deployment that archives.
pub fn consider(at: DateTime<Utc>, deployment: &Deployment) -> Result<(), DrillSkipped> {
    let window = deployment
        .maintenance_window
        .as_ref()
        .ok_or(DrillSkipped::NoWindow)?;

    if !window.contains(at) {
        return Err(DrillSkipped::OutsideWindow);
    }

    if let Some(last) = deployment.last_verified_restore_at
        && at - last < DRILL_INTERVAL
    {
        return Err(DrillSkipped::RecentlyVerified);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveTime, TimeZone, Weekday};
    use chrono_tz::UTC;
    use uuid::Uuid;

    use super::*;
    use crate::{
        dataplane::value_objects::{DataPlaneId, DeploymentResources},
        deployments::{
            DeploymentId, DeploymentKind, DeploymentName, DeploymentStatus,
            environment::Environment, network::NetworkAccess,
        },
        organisation::OrganisationId,
        upgrades::policy::MaintenanceWindow,
        user::UserId,
        version::Version,
    };

    fn deployment(window: Option<MaintenanceWindow>) -> Deployment {
        let at = Utc.with_ymd_and_hms(2026, 1, 5, 0, 0, 0).unwrap(); // a Monday
        Deployment {
            id: DeploymentId(Uuid::new_v4()),
            organisation_id: OrganisationId(Uuid::new_v4()),
            dataplane_id: DataPlaneId(Uuid::new_v4()),
            name: DeploymentName("auth".to_string()),
            kind: DeploymentKind::Ferriskey,
            version: Version::new(26, 0, 1),
            status: DeploymentStatus::Successful,
            namespace: "ns".to_string(),
            environment: Environment::Production,
            offer: None,
            restored_from: None,
            resources: DeploymentResources::DEFAULT,
            created_by: UserId(Uuid::new_v4()),
            created_at: at,
            updated_at: at,
            deployed_at: None,
            deleted_at: None,
            auto_upgrade: Default::default(),
            maintenance_window: window,
            network_access: NetworkAccess::Open,
            last_verified_restore_at: None,
            last_restore_drill_seconds: None,
        }
    }

    fn window_open_monday_midnight() -> MaintenanceWindow {
        MaintenanceWindow::new(
            Weekday::Mon,
            NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            Duration::hours(2),
            UTC,
        )
        .unwrap()
    }

    #[test]
    fn a_deployment_with_no_window_is_never_drilled_unattended() {
        let at = Utc.with_ymd_and_hms(2026, 1, 5, 0, 30, 0).unwrap();

        assert_eq!(consider(at, &deployment(None)), Err(DrillSkipped::NoWindow));
    }

    #[test]
    fn a_deployment_outside_its_window_is_passed_over() {
        let at = Utc.with_ymd_and_hms(2026, 1, 5, 12, 0, 0).unwrap();

        assert_eq!(
            consider(at, &deployment(Some(window_open_monday_midnight()))),
            Err(DrillSkipped::OutsideWindow)
        );
    }

    #[test]
    fn a_deployment_inside_its_window_with_no_prior_drill_is_due() {
        let at = Utc.with_ymd_and_hms(2026, 1, 5, 0, 30, 0).unwrap();

        assert_eq!(
            consider(at, &deployment(Some(window_open_monday_midnight()))),
            Ok(())
        );
    }

    #[test]
    fn a_deployment_verified_within_the_interval_is_passed_over() {
        let at = Utc.with_ymd_and_hms(2026, 1, 5, 0, 30, 0).unwrap();
        let mut deployment = deployment(Some(window_open_monday_midnight()));
        deployment.last_verified_restore_at = Some(at - Duration::days(2));

        assert_eq!(
            consider(at, &deployment),
            Err(DrillSkipped::RecentlyVerified)
        );
    }

    #[test]
    fn a_deployment_verified_longer_ago_than_the_interval_is_due_again() {
        let at = Utc.with_ymd_and_hms(2026, 1, 5, 0, 30, 0).unwrap();
        let mut deployment = deployment(Some(window_open_monday_midnight()));
        deployment.last_verified_restore_at = Some(at - Duration::days(8));

        assert_eq!(consider(at, &deployment), Ok(()));
    }
}
