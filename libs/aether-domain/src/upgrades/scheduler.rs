//! Deciding, at one moment, which deployments may be upgraded without asking.
//!
//! Pure. Time is a parameter and the estate is an argument, so the whole of
//! this can be tested without waiting for Sunday and without a database. That
//! is not a convenience: a scheduler whose tests sleep is a scheduler nobody
//! runs, and this one decides to change customers' software unattended.

use chrono::{DateTime, Duration, Utc};

use crate::{
    catalog::Release,
    deployments::{Deployment, DeploymentStatus},
    upgrades::policy::AutoUpgradePolicy,
    version::Version,
};

/// A deployment the scheduler is willing to start now, and where to take it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueUpgrade {
    pub deployment_id: crate::deployments::DeploymentId,
    pub from: Version,
    pub to: Version,
}

/// Why a deployment was passed over.
///
/// Carried rather than dropped because "nothing happened last night" is the
/// question a customer actually asks, and an empty list cannot answer it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Skipped {
    /// Nothing the catalogue offers is ahead of what it runs.
    AlreadyCurrent,
    /// The step exists and the policy does not cover it. A major always lands
    /// here, whatever the policy says.
    PolicyDeclines,
    /// No window, so no unattended action, whatever the policy says.
    NoWindow,
    /// The window is shut at this moment.
    OutsideWindow,
    /// Inside the window, but not enough of it left to finish.
    WouldNotFinish,
    /// Not settled: coming up, already upgrading, or being torn down.
    NotSettled,
    /// The previous attempt failed. Retrying unattended what just failed is
    /// the fastest way to turn an incident into an outage.
    PreviousAttemptFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passed {
    pub deployment_id: crate::deployments::DeploymentId,
    pub reason: Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Schedule {
    pub due: Vec<DueUpgrade>,
    pub passed: Vec<Passed>,
}

/// What the scheduler needs to know about a deployment beyond the deployment
/// itself.
///
/// Passed in rather than looked up, so this stays a function of its inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EstateFacts {
    /// Whether the most recent attempt on this deployment ended in failure.
    pub last_attempt_failed: bool,
    /// How long an upgrade is expected to take, used against what is left of
    /// the window.
    pub estimated: Duration,
}

/// Chooses what may start now.
///
/// `offers` is what the catalogue is willing to give each deployment, already
/// filtered for eligibility. Rollout is not decided here: who a release is
/// offered to is the catalogue's question, and answering it twice is how two
/// answers drift apart.
pub fn due(at: DateTime<Utc>, estate: &[(Deployment, EstateFacts, Vec<Release>)]) -> Schedule {
    let mut schedule = Schedule::default();

    for (deployment, facts, offers) in estate {
        let reason = match consider(at, deployment, facts, offers) {
            Ok(due) => {
                schedule.due.push(due);
                continue;
            }
            Err(reason) => reason,
        };

        schedule.passed.push(Passed {
            deployment_id: deployment.id,
            reason,
        });
    }

    schedule
}

fn consider(
    at: DateTime<Utc>,
    deployment: &Deployment,
    facts: &EstateFacts,
    offers: &[Release],
) -> Result<DueUpgrade, Skipped> {
    // Checked before anything else, and before the window: a deployment that
    // is upgrading, coming up or being torn down must not be looked at twice,
    // and the cheapest check is the one that rules that out.
    if deployment.status != DeploymentStatus::Successful {
        return Err(Skipped::NotSettled);
    }

    if facts.last_attempt_failed {
        return Err(Skipped::PreviousAttemptFailed);
    }

    // The best step the policy will take, not the newest release. Offering the
    // newest and then declining it would pass over a patch the customer did
    // agree to.
    let target = offers
        .iter()
        .filter(|release| release.id.version > deployment.version)
        .filter(|release| {
            deployment
                .version
                .change_to(&release.id.version)
                .is_ok_and(|change| deployment.auto_upgrade.allows(change))
        })
        .max_by(|a, b| a.id.version.cmp(&b.id.version));

    let Some(target) = target else {
        // Telling the two apart matters: one is a customer who is up to date,
        // the other is a customer whose policy is quietly refusing everything
        // on offer.
        let anything_ahead = offers
            .iter()
            .any(|release| release.id.version > deployment.version);

        return Err(if anything_ahead {
            Skipped::PolicyDeclines
        } else {
            Skipped::AlreadyCurrent
        });
    };

    let Some(window) = deployment.maintenance_window.as_ref() else {
        return Err(Skipped::NoWindow);
    };

    if !window.contains(at) {
        return Err(Skipped::OutsideWindow);
    }

    if !window.fits(at, facts.estimated) {
        return Err(Skipped::WouldNotFinish);
    }

    Ok(DueUpgrade {
        deployment_id: deployment.id,
        from: deployment.version.clone(),
        to: target.id.version.clone(),
    })
}

/// Whether a policy would take any step at all. Used to explain a deployment
/// that is passed over rather than to decide anything.
pub fn delegates_anything(policy: AutoUpgradePolicy) -> bool {
    policy != AutoUpgradePolicy::Manual
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        catalog::{BreakingRisk, ReleaseId, ReleaseNotes, ReleaseStatus},
        dataplane::value_objects::{DataPlaneId, DeploymentResources},
        deployments::{DeploymentId, DeploymentKind, DeploymentName},
        organisation::OrganisationId,
        upgrades::policy::MaintenanceWindow,
        user::UserId,
    };
    use chrono::{NaiveTime, Weekday};
    use chrono_tz::Tz;
    use uuid::Uuid;

    fn at(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .expect("a valid instant")
            .with_timezone(&Utc)
    }

    /// Sunday 03:00 UTC for two hours, and a Sunday to go with it.
    fn window() -> MaintenanceWindow {
        MaintenanceWindow::new(
            Weekday::Sun,
            NaiveTime::parse_from_str("03:00", "%H:%M").expect("valid"),
            Duration::hours(2),
            Tz::UTC,
        )
        .expect("a valid window")
    }

    fn inside() -> DateTime<Utc> {
        at("2026-01-04T03:10:00Z")
    }

    fn deployment(
        version: Version,
        policy: AutoUpgradePolicy,
        window: Option<MaintenanceWindow>,
    ) -> Deployment {
        let moment = inside();
        Deployment {
            id: DeploymentId(Uuid::from_u128(1)),
            organisation_id: OrganisationId(Uuid::from_u128(2)),
            dataplane_id: DataPlaneId(Uuid::from_u128(3)),
            name: DeploymentName("auth".to_string()),
            kind: DeploymentKind::Ferriskey,
            version,
            status: DeploymentStatus::Successful,
            namespace: "ns".to_string(),
            resources: DeploymentResources::DEFAULT,
            created_by: UserId(Uuid::from_u128(4)),
            created_at: moment,
            updated_at: moment,
            deployed_at: None,
            deleted_at: None,
            auto_upgrade: policy,
            maintenance_window: window,
            network_access: crate::deployments::network::NetworkAccess::Open,
        }
    }

    fn release(version: Version) -> Release {
        let mut release = Release::announce(
            ReleaseId::new(DeploymentKind::Ferriskey, version),
            BreakingRisk::None,
            ReleaseNotes("notes".to_string()),
            inside(),
        );
        release.status = ReleaseStatus::Available;
        release
    }

    fn facts() -> EstateFacts {
        EstateFacts {
            last_attempt_failed: false,
            estimated: Duration::minutes(20),
        }
    }

    fn schedule_of(
        moment: DateTime<Utc>,
        deployment: Deployment,
        facts: EstateFacts,
        offers: Vec<Release>,
    ) -> Schedule {
        due(moment, &[(deployment, facts, offers)])
    }

    fn only_reason(schedule: &Schedule) -> &Skipped {
        assert!(schedule.due.is_empty(), "{:?}", schedule.due);
        &schedule.passed.first().expect("one deployment").reason
    }

    #[test]
    fn a_patch_inside_the_window_is_due() {
        let schedule = schedule_of(
            inside(),
            deployment(
                Version::new(26, 0, 0),
                AutoUpgradePolicy::Patch,
                Some(window()),
            ),
            facts(),
            vec![release(Version::new(26, 0, 1))],
        );

        assert_eq!(schedule.due.len(), 1);
        assert_eq!(schedule.due[0].to, Version::new(26, 0, 1));
        assert_eq!(schedule.due[0].from, Version::new(26, 0, 0));
    }

    /// Time is a parameter, which is the whole reason this can be tested at
    /// all. Nothing here waits for a Sunday.
    #[test]
    fn nothing_is_due_outside_the_window() {
        let schedule = schedule_of(
            at("2026-01-05T03:10:00Z"),
            deployment(
                Version::new(26, 0, 0),
                AutoUpgradePolicy::Patch,
                Some(window()),
            ),
            facts(),
            vec![release(Version::new(26, 0, 1))],
        );

        assert_eq!(only_reason(&schedule), &Skipped::OutsideWindow);
    }

    /// No window means never, whatever the policy says. Declining to name one
    /// is a choice, and a default window would be an automation nobody asked
    /// for.
    #[test]
    fn a_deployment_with_no_window_is_never_due() {
        let schedule = schedule_of(
            inside(),
            deployment(
                Version::new(26, 0, 0),
                AutoUpgradePolicy::PatchAndMinor,
                None,
            ),
            facts(),
            vec![
                release(Version::new(27, 0, 0)),
                release(Version::new(26, 1, 0)),
            ],
        );

        assert_eq!(only_reason(&schedule), &Skipped::NoWindow);
    }

    /// A rolling upgrade cut off by the end of a window is not paused, it is a
    /// half-upgraded instance in working hours.
    #[test]
    fn what_cannot_finish_before_the_window_shuts_is_not_started() {
        let schedule = schedule_of(
            at("2026-01-04T04:50:00Z"),
            deployment(
                Version::new(26, 0, 0),
                AutoUpgradePolicy::Patch,
                Some(window()),
            ),
            EstateFacts {
                estimated: Duration::minutes(20),
                ..facts()
            },
            vec![release(Version::new(26, 0, 1))],
        );

        assert_eq!(only_reason(&schedule), &Skipped::WouldNotFinish);
    }

    /// Retrying unattended what just failed is the fastest way to turn an
    /// incident into an outage.
    #[test]
    fn a_deployment_whose_last_attempt_failed_is_never_due() {
        let schedule = schedule_of(
            inside(),
            deployment(
                Version::new(26, 0, 0),
                AutoUpgradePolicy::Patch,
                Some(window()),
            ),
            EstateFacts {
                last_attempt_failed: true,
                ..facts()
            },
            vec![release(Version::new(26, 0, 1))],
        );

        assert_eq!(only_reason(&schedule), &Skipped::PreviousAttemptFailed);
    }

    #[test]
    fn a_deployment_that_is_not_settled_is_never_due() {
        for status in [
            DeploymentStatus::Pending,
            DeploymentStatus::InProgress,
            DeploymentStatus::Upgrading,
            DeploymentStatus::Failed,
            DeploymentStatus::Deleting,
            DeploymentStatus::Deleted,
        ] {
            let mut subject = deployment(
                Version::new(26, 0, 0),
                AutoUpgradePolicy::Patch,
                Some(window()),
            );
            subject.status = status.clone();

            let schedule = schedule_of(
                inside(),
                subject,
                facts(),
                vec![release(Version::new(26, 0, 1))],
            );

            assert_eq!(
                only_reason(&schedule),
                &Skipped::NotSettled,
                "{status:?} must not be scheduled"
            );
        }
    }

    /// The invariant the whole feature rests on. A major is on offer, the
    /// window is open, the policy is the most permissive there is, and it
    /// still does not go.
    #[test]
    fn a_major_is_never_due_however_permissive_the_policy() {
        let schedule = schedule_of(
            inside(),
            deployment(
                Version::new(26, 0, 0),
                AutoUpgradePolicy::PatchAndMinor,
                Some(window()),
            ),
            facts(),
            vec![release(Version::new(27, 0, 0))],
        );

        assert_eq!(only_reason(&schedule), &Skipped::PolicyDeclines);
    }

    /// Taking the newest and then declining it would pass over a patch the
    /// customer did agree to.
    #[test]
    fn the_best_step_the_policy_accepts_is_chosen_not_the_newest_release() {
        let schedule = schedule_of(
            inside(),
            deployment(
                Version::new(26, 0, 0),
                AutoUpgradePolicy::Patch,
                Some(window()),
            ),
            facts(),
            vec![
                release(Version::new(27, 0, 0)),
                release(Version::new(26, 0, 2)),
                release(Version::new(26, 0, 1)),
            ],
        );

        assert_eq!(schedule.due.len(), 1);
        assert_eq!(schedule.due[0].to, Version::new(26, 0, 2));
    }

    /// A customer who is up to date and one whose policy is refusing
    /// everything look identical from outside, and only one of them has a
    /// setting to change.
    #[test]
    fn being_current_is_reported_differently_from_being_declined() {
        let current = schedule_of(
            inside(),
            deployment(
                Version::new(26, 0, 1),
                AutoUpgradePolicy::Patch,
                Some(window()),
            ),
            facts(),
            vec![release(Version::new(26, 0, 1))],
        );
        assert_eq!(only_reason(&current), &Skipped::AlreadyCurrent);

        let declined = schedule_of(
            inside(),
            deployment(
                Version::new(26, 0, 0),
                AutoUpgradePolicy::Manual,
                Some(window()),
            ),
            facts(),
            vec![release(Version::new(26, 0, 1))],
        );
        assert_eq!(only_reason(&declined), &Skipped::PolicyDeclines);
    }

    #[test]
    fn every_deployment_is_accounted_for() {
        let ready = deployment(
            Version::new(26, 0, 0),
            AutoUpgradePolicy::Patch,
            Some(window()),
        );
        let mut without_window = ready.clone();
        without_window.id = DeploymentId(Uuid::from_u128(9));
        without_window.maintenance_window = None;

        let schedule = due(
            inside(),
            &[
                (ready, facts(), vec![release(Version::new(26, 0, 1))]),
                (
                    without_window,
                    facts(),
                    vec![release(Version::new(26, 0, 1))],
                ),
            ],
        );

        assert_eq!(schedule.due.len() + schedule.passed.len(), 2);
    }
}
