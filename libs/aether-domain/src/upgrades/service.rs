use chrono::Utc;
use serde_json::json;
use tracing::info;

use aether_auth::Identity;

use crate::{
    CoreError,
    audit::{AuditAction, AuditChange},
    catalog::{RolloutCandidate, ports::ReleaseRepository},
    deployments::{Deployment, DeploymentId, DeploymentStatus, ports::DeploymentRepository},
    organisation::ports::OrganisationRepository,
    upgrades::{
        commands::{RequestUpgradeCommand, SetUpgradeSettingsCommand},
        path::UpgradePath,
        policy::{AutoUpgradePolicy, MaintenanceWindow},
        ports::{AcceptedUpgrade, UpgradePolicy, UpgradeProgress, UpgradeService},
        run::{InFlightUpgrade, UpgradeRun},
        run_ports::UpgradeRunRepository,
    },
    version::Version,
};

/// A deployment as it stood before its upgrade settings were written, and as
/// it stands after.
///
/// Handed back by an inherent method rather than by [`UpgradeService`], whose
/// shape every caller sees: only the caller that records the change needs the
/// value that was overwritten, and the row stops holding it the moment the
/// update lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedUpgradeSettings {
    pub before: Deployment,
    pub after: Deployment,
}

/// One statement about a configuration change, ready for the audit trail once
/// the caller has resolved who made it.
///
/// Written here rather than in the application layer because deciding what a
/// setting means and deciding how it is written down are the same knowledge.
/// Split across two crates, the two drift, and the trail ends up describing a
/// setting that no longer works that way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigurationChange {
    pub action: AuditAction,
    pub change: AuditChange,
}

impl ConfigurationChange {
    /// Someone accepted moving a deployment to another version.
    ///
    /// The end of the path, not the first step: the stepping stones in between
    /// are the platform's decision, and the trail records the one the caller
    /// made.
    pub fn upgrade_approved(accepted: &AcceptedUpgrade) -> Result<Self, CoreError> {
        let target = accepted
            .path
            .steps()
            .last()
            .expect("a path always has a last step");

        Ok(Self {
            action: AuditAction("deployment.upgrade.approved".to_string()),
            change: AuditChange::new(
                json!({ "version": accepted.deployment.version.to_string() }),
                json!({ "version": target.to_string() }),
            )?,
        })
    }
}

impl AppliedUpgradeSettings {
    /// One statement per setting that actually moved.
    ///
    /// A write that changes nothing leaves nothing behind: an entry saying a
    /// value went from `manual` to `manual` is noise in the one place where
    /// noise hides the entry somebody is looking for.
    ///
    /// Every leaf in these statements is a scalar this platform defines: one
    /// of three policy names, a weekday, a local time, a count of minutes, an
    /// IANA zone name, a semver string. No struct is serialised whole and
    /// nothing is read out of a customer's own realm, so the shape has no
    /// field in which a token, a password or a connection string could arrive.
    pub fn configuration_changes(&self) -> Result<Vec<ConfigurationChange>, CoreError> {
        let mut changes = Vec::new();

        if self.before.auto_upgrade != self.after.auto_upgrade {
            changes.push(ConfigurationChange {
                action: AuditAction("deployment.auto_upgrade.updated".to_string()),
                change: AuditChange::new(
                    described_policy(self.before.auto_upgrade),
                    described_policy(self.after.auto_upgrade),
                )?,
            });
        }

        if self.before.maintenance_window != self.after.maintenance_window {
            changes.push(ConfigurationChange {
                action: AuditAction("deployment.maintenance_window.updated".to_string()),
                change: AuditChange::new(
                    described_window(self.before.maintenance_window.as_ref()),
                    described_window(self.after.maintenance_window.as_ref()),
                )?,
            });
        }

        Ok(changes)
    }
}

fn described_policy(policy: AutoUpgradePolicy) -> serde_json::Value {
    json!({ "auto_upgrade": policy })
}

/// The window written out field by field rather than serialised as a struct.
///
/// Naming each field is what keeps the statement honest as the type grows: a
/// field added to [`MaintenanceWindow`] later does not reach the audit trail
/// until somebody names it here and decides it belongs.
fn described_window(window: Option<&MaintenanceWindow>) -> serde_json::Value {
    let described = window.map(|window| {
        json!({
            "day": window.day.to_string(),
            "start": window.start.format("%H:%M").to_string(),
            "duration_minutes": window.duration.num_minutes(),
            "timezone": window.timezone.name(),
        })
    });

    json!({ "maintenance_window": described })
}

pub struct UpgradeServiceImpl<D, R, U, O, P>
where
    D: DeploymentRepository,
    R: ReleaseRepository,
    U: UpgradeRunRepository,
    O: OrganisationRepository,
    P: UpgradePolicy,
{
    deployment_repository: D,
    release_repository: R,
    run_repository: U,
    organisation_repository: O,
    policy: P,
}

impl<D, R, U, O, P> UpgradeServiceImpl<D, R, U, O, P>
where
    D: DeploymentRepository,
    R: ReleaseRepository,
    U: UpgradeRunRepository,
    O: OrganisationRepository,
    P: UpgradePolicy,
{
    pub fn new(
        deployment_repository: D,
        release_repository: R,
        run_repository: U,
        organisation_repository: O,
        policy: P,
    ) -> Self {
        Self {
            deployment_repository,
            release_repository,
            run_repository,
            organisation_repository,
            policy,
        }
    }

    /// The run a deployment is in the middle of, if any.
    ///
    /// Newest first from the repository, so the first unconcluded one is the
    /// current attempt. There is never more than one: an upgrade is refused
    /// while a deployment is not settled.
    async fn open_run(&self, deployment_id: DeploymentId) -> Result<Option<UpgradeRun>, CoreError> {
        Ok(self
            .run_repository
            .list_for_deployment(deployment_id)
            .await?
            .into_iter()
            .find(|run| !run.is_concluded()))
    }

    /// What [`UpgradeService::set_upgrade_settings`] does, keeping the value it
    /// overwrote.
    ///
    /// The read already happens -- the settings are written onto the
    /// deployment that was fetched -- so holding on to that copy costs
    /// nothing, and it is the only moment the old value still exists anywhere.
    pub async fn apply_upgrade_settings(
        &self,
        identity: Identity,
        command: SetUpgradeSettingsCommand,
    ) -> Result<AppliedUpgradeSettings, CoreError> {
        self.policy
            .can_upgrade_deployment(identity, command.organisation_id)
            .await?;

        let before = self
            .deployment_repository
            .get_by_id(command.deployment_id)
            .await?
            .filter(|deployment| deployment.organisation_id == command.organisation_id)
            .ok_or(CoreError::DeploymentNotFound {
                id: command.deployment_id.0,
            })?;

        let mut after = before.clone();
        after.auto_upgrade = command.auto_upgrade;
        after.maintenance_window = command.maintenance_window;
        after.updated_at = Utc::now();
        self.deployment_repository.update(after.clone()).await?;

        Ok(AppliedUpgradeSettings { before, after })
    }
}

impl<D, R, U, O, P> UpgradeService for UpgradeServiceImpl<D, R, U, O, P>
where
    D: DeploymentRepository,
    R: ReleaseRepository,
    U: UpgradeRunRepository,
    O: OrganisationRepository,
    P: UpgradePolicy,
{
    async fn set_upgrade_settings(
        &self,
        identity: Identity,
        command: SetUpgradeSettingsCommand,
    ) -> Result<Deployment, CoreError> {
        Ok(self.apply_upgrade_settings(identity, command).await?.after)
    }

    async fn request_upgrade(
        &self,
        identity: Identity,
        command: RequestUpgradeCommand,
    ) -> Result<AcceptedUpgrade, CoreError> {
        // Before the deployment is even read. A caller with no right to move
        // this organisation's versions must not learn whether a given id
        // exists, and must certainly not learn what it is running.
        self.policy
            .can_upgrade_deployment(identity, command.organisation_id)
            .await?;

        let mut deployment = self
            .deployment_repository
            .get_by_id(command.deployment_id)
            .await?
            .filter(|deployment| deployment.organisation_id == command.organisation_id)
            .ok_or(CoreError::DeploymentNotFound {
                id: command.deployment_id.0,
            })?;

        // Settled first, before anything is looked up. Upgrading something
        // that is still coming up, already upgrading, or being torn down puts
        // two operations on one instance, and the second one wins by accident.
        if deployment.status != DeploymentStatus::Successful {
            return Err(CoreError::DeploymentNotUpgradable {
                deployment: deployment.id.0,
                status: deployment.status.to_string(),
            });
        }

        // Checked before the step is classified: a version the catalogue does
        // not hold is a different mistake from one that is not ahead, and
        // saying "not ahead" about a version that does not exist sends the
        // reader looking in the wrong place.
        let release = self
            .release_repository
            .get(&deployment.kind, &command.target)
            .await?
            .ok_or_else(|| CoreError::ReleaseNotFound {
                release: format!("{} {}", deployment.kind, command.target),
            })?;

        if !release.status.is_installable() {
            return Err(CoreError::ReleaseNotInstallable {
                release: release.id.to_string(),
                status: format!("{:?}", release.status).to_lowercase(),
            });
        }

        // Being installable and being offered here are different questions. A
        // release reaches an estate by degrees, and asking for one by name
        // must not be the way around the step it has reached: the screen
        // already declines to show it, and a request that bypassed the screen
        // would be the only path that ignores the rollout.
        let organisation = self
            .organisation_repository
            .find_by_id(&deployment.organisation_id)
            .await?
            .ok_or(CoreError::OrganisationNotFound {
                id: deployment.organisation_id.0,
            })?;

        let candidate = RolloutCandidate {
            deployment_id: deployment.id,
            organisation_id: deployment.organisation_id,
            plan: organisation.plan,
        };

        if !release.rollout.covers(&release.id, &candidate) {
            return Err(CoreError::ReleaseNotOffered {
                release: release.id.to_string(),
            });
        }

        // Where the no downgrade rule lives: on the step, not on the version
        // type. On the type it would also forbid the rollback a failed upgrade
        // needs, which restores a version the deployment came from.
        let change = deployment.version.change_to(&command.target)?;

        // Planned against the catalogue as it stands now, and carried with the
        // acceptance. Re-planning later would let a release withdrawn halfway
        // through reroute an upgrade already under way.
        let catalogue = self
            .release_repository
            .list_for_kind(&deployment.kind)
            .await?;
        let path = UpgradePath::plan(&deployment.version, &release, &catalogue)?;

        info!(
            deployment_id = %deployment.id,
            from = %deployment.version,
            to = %command.target,
            change = ?change,
            steps = path.len(),
            "accepting an upgrade"
        );

        deployment.status = DeploymentStatus::Upgrading;
        deployment.updated_at = Utc::now();
        self.deployment_repository
            .update(deployment.clone())
            .await?;

        Ok(AcceptedUpgrade {
            deployment,
            change,
            path,
        })
    }

    async fn upgrade_in_flight(
        &self,
        organisation_id: crate::organisation::OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<Option<InFlightUpgrade>, CoreError> {
        let deployment = self
            .deployment_repository
            .get_by_id(deployment_id)
            .await?
            .filter(|deployment| deployment.organisation_id == organisation_id)
            .ok_or(CoreError::DeploymentNotFound {
                id: deployment_id.0,
            })?;

        Ok(self
            .open_run(deployment_id)
            .await?
            .map(|run| run.in_flight(deployment.version)))
    }

    async fn advance_upgrade(
        &self,
        deployment_id: DeploymentId,
    ) -> Result<UpgradeProgress, CoreError> {
        let Some(mut run) = self.open_run(deployment_id).await? else {
            return Ok(UpgradeProgress::Untouched);
        };

        let Some(mut deployment) = self.deployment_repository.get_by_id(deployment_id).await?
        else {
            // The deployment went away under an upgrade. Nothing left to move,
            // and the run is closed rather than left open for ever.
            run.fail("the deployment no longer exists", Utc::now())?;
            self.run_repository.update(&run).await?;
            return Ok(UpgradeProgress::GaveUp);
        };

        let now = Utc::now();

        match deployment.status {
            // Reported unwell. Applying the next step to an instance that is
            // already failing turns one bad version into two.
            DeploymentStatus::Failed => {
                run.fail(
                    format!("the deployment came back failed on {}", deployment.version),
                    now,
                )?;
                self.run_repository.update(&run).await?;
                Ok(UpgradeProgress::GaveUp)
            }

            // Settled on a version. Either that is the end of the path, or the
            // step that just landed was one of several.
            DeploymentStatus::Successful => {
                let Some(next) = next_step(&run, &deployment.version) else {
                    run.succeed(now)?;
                    self.run_repository.update(&run).await?;
                    return Ok(UpgradeProgress::Arrived);
                };

                info!(
                    deployment_id = %deployment.id,
                    at = %deployment.version,
                    next = %next,
                    "a step landed, applying the next one"
                );

                deployment.status = DeploymentStatus::Upgrading;
                deployment.updated_at = now;
                self.deployment_repository
                    .update(deployment.clone())
                    .await?;

                Ok(UpgradeProgress::NextStep {
                    deployment: Box::new(deployment),
                    to: next,
                })
            }

            // Still moving, or being torn down. Neither says anything about
            // the step, so neither closes the run.
            _ => Ok(UpgradeProgress::Untouched),
        }
    }
}

/// The first version in the run's path that the deployment has not reached.
fn next_step(run: &UpgradeRun, current: &Version) -> Option<Version> {
    run.steps.iter().find(|step| *step > current).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upgrades::policy::AutoUpgradePolicy;
    use crate::upgrades::run::{UpgradeRunId, UpgradeTrigger};
    use crate::{
        catalog::{BreakingRisk, Release, ReleaseId, ReleaseNotes, ReleaseStatus, Rollout},
        dataplane::value_objects::{DataPlaneId, DeploymentResources},
        deployments::{
            Deployment, DeploymentId, DeploymentKind, DeploymentName,
            ports::MockDeploymentRepository,
        },
        organisation::OrganisationId,
        organisation::{Organisation, ports::MockOrganisationRepository, value_objects::Plan},
        user::UserId,
        version::{Version, VersionChange, VersionError},
    };
    use chrono::{Duration, NaiveTime, Weekday};
    use chrono_tz::Tz;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    #[derive(Clone)]
    struct StubReleases(Arc<Mutex<Vec<Release>>>);

    impl StubReleases {
        fn holding(releases: Vec<Release>) -> Self {
            Self(Arc::new(Mutex::new(releases)))
        }
    }

    impl ReleaseRepository for StubReleases {
        async fn insert(&self, _release: Release) -> Result<(), CoreError> {
            unreachable!("an upgrade never writes to the catalogue")
        }

        async fn get(
            &self,
            kind: &DeploymentKind,
            version: &Version,
        ) -> Result<Option<Release>, CoreError> {
            Ok(self
                .0
                .lock()
                .expect("not poisoned")
                .iter()
                .find(|release| &release.id.kind == kind && &release.id.version == version)
                .cloned())
        }

        async fn list_for_kind(&self, kind: &DeploymentKind) -> Result<Vec<Release>, CoreError> {
            Ok(self
                .0
                .lock()
                .expect("not poisoned")
                .iter()
                .filter(|release| &release.id.kind == kind)
                .cloned()
                .collect())
        }

        async fn update(&self, _release: &Release) -> Result<(), CoreError> {
            unreachable!("an upgrade never writes to the catalogue")
        }
    }

    /// Holds whatever runs a test needs and keeps what was written, so
    /// "recorded" and "concluded" can both be asserted on.
    #[derive(Clone, Default)]
    struct StubRuns {
        runs: Arc<Mutex<Vec<UpgradeRun>>>,
    }

    impl StubRuns {
        fn empty() -> Self {
            Self::default()
        }

        fn holding(runs: Vec<UpgradeRun>) -> Self {
            Self {
                runs: Arc::new(Mutex::new(runs)),
            }
        }

        fn only(&self) -> UpgradeRun {
            self.runs
                .lock()
                .expect("not poisoned")
                .first()
                .cloned()
                .expect("a run was recorded")
        }
    }

    impl UpgradeRunRepository for StubRuns {
        async fn insert(&self, run: UpgradeRun) -> Result<(), CoreError> {
            self.runs.lock().expect("not poisoned").push(run);
            Ok(())
        }

        async fn get(
            &self,
            id: crate::upgrades::run::UpgradeRunId,
        ) -> Result<Option<UpgradeRun>, CoreError> {
            Ok(self
                .runs
                .lock()
                .expect("not poisoned")
                .iter()
                .find(|run| run.id == id)
                .cloned())
        }

        async fn update(&self, run: &UpgradeRun) -> Result<(), CoreError> {
            let mut runs = self.runs.lock().expect("not poisoned");
            let Some(slot) = runs.iter_mut().find(|held| held.id == run.id) else {
                return Err(CoreError::UpgradeRunNotFound { id: run.id.0 });
            };
            *slot = run.clone();
            Ok(())
        }

        async fn list_for_deployment(
            &self,
            deployment_id: DeploymentId,
        ) -> Result<Vec<UpgradeRun>, CoreError> {
            Ok(self
                .runs
                .lock()
                .expect("not poisoned")
                .iter()
                .filter(|run| run.deployment_id == deployment_id)
                .cloned()
                .collect())
        }
    }

    /// An organisation for the deployment under test. Its plan is what the
    /// rollout reads, so it has to be a real one rather than a default.
    fn organisations() -> MockOrganisationRepository {
        let mut mock = MockOrganisationRepository::new();
        mock.expect_find_by_id().returning(|id| {
            let mut organisation = Organisation::new(
                crate::organisation::value_objects::OrganisationName::new("FerrisLabs")
                    .expect("a name"),
                crate::organisation::value_objects::OrganisationSlug::new("ferrislabs")
                    .expect("a slug"),
                UserId(Uuid::from_u128(7)),
                Plan::Free,
            );
            organisation.id = *id;
            Box::pin(async move { Ok(Some(organisation)) })
        });
        mock
    }

    /// Grants or refuses, and records that it was asked. A permission check
    /// that is never reached is the failure mode worth testing for.
    #[derive(Clone)]
    struct StubPolicy {
        allowed: bool,
        asked: Arc<Mutex<usize>>,
    }

    impl StubPolicy {
        fn allowing() -> Self {
            Self {
                allowed: true,
                asked: Arc::new(Mutex::new(0)),
            }
        }

        fn refusing() -> Self {
            Self {
                allowed: false,
                asked: Arc::new(Mutex::new(0)),
            }
        }
    }

    impl UpgradePolicy for StubPolicy {
        async fn can_upgrade_deployment(
            &self,
            _identity: Identity,
            _organisation_id: crate::organisation::OrganisationId,
        ) -> Result<(), CoreError> {
            *self.asked.lock().expect("not poisoned") += 1;
            if self.allowed {
                Ok(())
            } else {
                Err(CoreError::PermissionDenied {
                    reason: "not allowed to upgrade".to_string(),
                })
            }
        }
    }

    fn caller() -> Identity {
        Identity::User(aether_auth::User {
            id: "user".to_string(),
            username: "user".to_string(),
            email: None,
            name: None,
            roles: vec![],
        })
    }

    /// A release that is offered to everyone. Published releases start
    /// offered to nobody, which is its own rule with its own tests below;
    /// every other test here is about something else and would otherwise be
    /// asserting that rule by accident.
    fn release(version: Version, status: ReleaseStatus) -> Release {
        let mut release = closed_release(version, status);
        release.rollout = Rollout::full();
        release
    }

    fn closed_release(version: Version, status: ReleaseStatus) -> Release {
        let mut release = Release::announce(
            ReleaseId::new(DeploymentKind::Ferriskey, version),
            BreakingRisk::None,
            ReleaseNotes("notes".to_string()),
            Utc::now(),
        );
        release.status = status;
        release
    }

    const ORGANISATION: Uuid = Uuid::from_u128(1);
    const DEPLOYMENT: Uuid = Uuid::from_u128(2);

    fn deployment(status: DeploymentStatus, version: Version) -> Deployment {
        let at = Utc::now();
        Deployment {
            id: DeploymentId(DEPLOYMENT),
            organisation_id: OrganisationId(ORGANISATION),
            dataplane_id: DataPlaneId(Uuid::from_u128(3)),
            name: DeploymentName("auth".to_string()),
            kind: DeploymentKind::Ferriskey,
            version,
            status,
            namespace: "ns".to_string(),
            resources: DeploymentResources::DEFAULT,
            created_by: UserId(Uuid::from_u128(4)),
            created_at: at,
            updated_at: at,
            deployed_at: None,
            deleted_at: None,
            auto_upgrade: Default::default(),
            maintenance_window: None,
            network_access: crate::deployments::network::NetworkAccess::Open,
        }
    }

    fn command(target: Version) -> RequestUpgradeCommand {
        RequestUpgradeCommand {
            organisation_id: OrganisationId(ORGANISATION),
            deployment_id: DeploymentId(DEPLOYMENT),
            target,
        }
    }

    /// Records what the service wrote, because "refused" and "refused without
    /// writing" are different guarantees and only the second one is safe.
    fn repository(
        found: Option<Deployment>,
        writes: Arc<Mutex<Vec<Deployment>>>,
    ) -> MockDeploymentRepository {
        let mut mock = MockDeploymentRepository::new();
        mock.expect_get_by_id().returning(move |_| {
            let found = found.clone();
            Box::pin(async move { Ok(found) })
        });
        mock.expect_update().returning(move |deployment| {
            writes.lock().expect("not poisoned").push(deployment);
            Box::pin(async { Ok(()) })
        });
        mock
    }

    #[tokio::test]
    async fn a_settled_deployment_moves_to_upgrading() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let service = UpgradeServiceImpl::new(
            repository(
                Some(deployment(
                    DeploymentStatus::Successful,
                    Version::new(26, 0, 0),
                )),
                writes.clone(),
            ),
            StubReleases::holding(vec![release(
                Version::new(26, 0, 1),
                ReleaseStatus::Available,
            )]),
            StubRuns::empty(),
            organisations(),
            StubPolicy::allowing(),
        );

        let accepted = service
            .request_upgrade(caller(), command(Version::new(26, 0, 1)))
            .await
            .expect("a settled deployment may upgrade");

        assert_eq!(accepted.change, VersionChange::Patch);
        assert_eq!(accepted.deployment.status, DeploymentStatus::Upgrading);
        assert_eq!(writes.lock().expect("not poisoned").len(), 1);
    }

    /// Upgrading something that is still coming up, already upgrading, or
    /// being torn down puts two operations on one instance and lets the second
    /// win by accident.
    #[tokio::test]
    async fn only_a_settled_deployment_can_be_upgraded() {
        for status in [
            DeploymentStatus::Pending,
            DeploymentStatus::InProgress,
            DeploymentStatus::Upgrading,
            DeploymentStatus::Failed,
            DeploymentStatus::Deleting,
            DeploymentStatus::Deleted,
        ] {
            let writes = Arc::new(Mutex::new(Vec::new()));
            let service = UpgradeServiceImpl::new(
                repository(
                    Some(deployment(status.clone(), Version::new(26, 0, 0))),
                    writes.clone(),
                ),
                StubReleases::holding(vec![release(
                    Version::new(26, 0, 1),
                    ReleaseStatus::Available,
                )]),
                StubRuns::empty(),
                organisations(),
                StubPolicy::allowing(),
            );

            let outcome = service
                .request_upgrade(caller(), command(Version::new(26, 0, 1)))
                .await;

            assert!(
                matches!(outcome, Err(CoreError::DeploymentNotUpgradable { .. })),
                "{status:?} must not accept an upgrade"
            );
            assert!(
                writes.lock().expect("not poisoned").is_empty(),
                "{status:?} was refused but the deployment was still written"
            );
        }
    }

    #[tokio::test]
    async fn the_refusal_names_the_status_that_blocked_it() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let service = UpgradeServiceImpl::new(
            repository(
                Some(deployment(
                    DeploymentStatus::Deleting,
                    Version::new(26, 0, 0),
                )),
                writes,
            ),
            StubReleases::holding(Vec::new()),
            StubRuns::empty(),
            organisations(),
            StubPolicy::allowing(),
        );

        let error = service
            .request_upgrade(caller(), command(Version::new(26, 0, 1)))
            .await
            .expect_err("a deployment being torn down");

        assert!(error.to_string().contains("deleting"), "{error}");
    }

    #[tokio::test]
    async fn a_target_the_catalogue_does_not_hold_is_refused() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let service = UpgradeServiceImpl::new(
            repository(
                Some(deployment(
                    DeploymentStatus::Successful,
                    Version::new(26, 0, 0),
                )),
                writes.clone(),
            ),
            StubReleases::holding(Vec::new()),
            StubRuns::empty(),
            organisations(),
            StubPolicy::allowing(),
        );

        let outcome = service
            .request_upgrade(caller(), command(Version::new(26, 0, 1)))
            .await;

        assert!(matches!(outcome, Err(CoreError::ReleaseNotFound { .. })));
        assert!(writes.lock().expect("not poisoned").is_empty());
    }

    /// Withdrawn means "must not be installed", and an upgrade is an install.
    /// Upcoming has not been published at all.
    #[tokio::test]
    async fn a_target_the_catalogue_will_not_install_is_refused() {
        for status in [ReleaseStatus::Withdrawn, ReleaseStatus::Upcoming] {
            let writes = Arc::new(Mutex::new(Vec::new()));
            let service = UpgradeServiceImpl::new(
                repository(
                    Some(deployment(
                        DeploymentStatus::Successful,
                        Version::new(26, 0, 0),
                    )),
                    writes.clone(),
                ),
                StubReleases::holding(vec![release(Version::new(26, 0, 1), status)]),
                StubRuns::empty(),
                organisations(),
                StubPolicy::allowing(),
            );

            let outcome = service
                .request_upgrade(caller(), command(Version::new(26, 0, 1)))
                .await;

            assert!(
                matches!(outcome, Err(CoreError::ReleaseNotInstallable { .. })),
                "{status:?} must not be an upgrade target"
            );
            assert!(writes.lock().expect("not poisoned").is_empty());
        }
    }

    /// Deprecated still runs and can still be moved to. It has to stay a valid
    /// target: an upgrade path with mandatory steps will pass through versions
    /// that have since been deprecated.
    /// The rule the whole rollout exists for. Asking for a version by name
    /// must not be the way around the step it has reached: the screen already
    /// declines to show it, and a request that bypassed the screen would be
    /// the only path in the platform that ignores the rollout.
    #[tokio::test]
    async fn a_version_offered_to_nobody_is_refused_even_when_asked_for_by_name() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let service = UpgradeServiceImpl::new(
            repository(
                Some(deployment(
                    DeploymentStatus::Successful,
                    Version::new(26, 0, 0),
                )),
                writes.clone(),
            ),
            StubReleases::holding(vec![closed_release(
                Version::new(26, 0, 1),
                ReleaseStatus::Available,
            )]),
            StubRuns::empty(),
            organisations(),
            StubPolicy::allowing(),
        );

        let error = service
            .request_upgrade(caller(), command(Version::new(26, 0, 1)))
            .await
            .expect_err("not offered here");

        assert!(
            matches!(error, CoreError::ReleaseNotOffered { .. }),
            "{error:?}"
        );
        assert!(
            writes.lock().expect("not poisoned").is_empty(),
            "a refused upgrade must not have moved the deployment"
        );
    }

    /// The same release, once it has been widened. Nothing else changes.
    #[tokio::test]
    async fn a_version_offered_to_everyone_is_accepted() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let mut offered = closed_release(Version::new(26, 0, 1), ReleaseStatus::Available);
        offered
            .widen_rollout(Rollout::full(), Utc::now())
            .expect("widening is allowed");

        let service = UpgradeServiceImpl::new(
            repository(
                Some(deployment(
                    DeploymentStatus::Successful,
                    Version::new(26, 0, 0),
                )),
                writes.clone(),
            ),
            StubReleases::holding(vec![offered]),
            StubRuns::empty(),
            organisations(),
            StubPolicy::allowing(),
        );

        service
            .request_upgrade(caller(), command(Version::new(26, 0, 1)))
            .await
            .expect("offered to everyone");
    }

    #[tokio::test]
    async fn a_deprecated_target_is_still_allowed() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let service = UpgradeServiceImpl::new(
            repository(
                Some(deployment(
                    DeploymentStatus::Successful,
                    Version::new(25, 0, 0),
                )),
                writes,
            ),
            StubReleases::holding(vec![release(
                Version::new(25, 4, 2),
                ReleaseStatus::Deprecated,
            )]),
            StubRuns::empty(),
            organisations(),
            StubPolicy::allowing(),
        );

        let accepted = service
            .request_upgrade(caller(), command(Version::new(25, 4, 2)))
            .await
            .expect("a deprecated version is a valid stepping stone");

        assert_eq!(accepted.change, VersionChange::Minor);
    }

    #[tokio::test]
    async fn a_target_that_is_not_ahead_is_refused() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let service = UpgradeServiceImpl::new(
            repository(
                Some(deployment(
                    DeploymentStatus::Successful,
                    Version::new(26, 1, 0),
                )),
                writes.clone(),
            ),
            StubReleases::holding(vec![release(
                Version::new(26, 0, 1),
                ReleaseStatus::Available,
            )]),
            StubRuns::empty(),
            organisations(),
            StubPolicy::allowing(),
        );

        let outcome = service
            .request_upgrade(caller(), command(Version::new(26, 0, 1)))
            .await;

        assert!(matches!(
            outcome,
            Err(CoreError::Version(VersionError::NotAhead { .. }))
        ));
        assert!(writes.lock().expect("not poisoned").is_empty());
    }

    /// A major is accepted here. Whether it may be applied without asking is
    /// the client policy's question in V4, and answering it twice is how the
    /// two answers drift apart.
    #[tokio::test]
    async fn a_major_is_accepted_and_reported_as_a_major() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let service = UpgradeServiceImpl::new(
            repository(
                Some(deployment(
                    DeploymentStatus::Successful,
                    Version::new(26, 0, 0),
                )),
                writes,
            ),
            StubReleases::holding(vec![release(
                Version::new(27, 0, 0),
                ReleaseStatus::Available,
            )]),
            StubRuns::empty(),
            organisations(),
            StubPolicy::allowing(),
        );

        let accepted = service
            .request_upgrade(caller(), command(Version::new(27, 0, 0)))
            .await
            .expect("accepted");

        assert_eq!(accepted.change, VersionChange::Major);
    }

    #[tokio::test]
    async fn a_deployment_of_another_organisation_is_not_found() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let service = UpgradeServiceImpl::new(
            repository(None, writes.clone()),
            StubReleases::holding(Vec::new()),
            StubRuns::empty(),
            organisations(),
            StubPolicy::allowing(),
        );

        let outcome = service
            .request_upgrade(caller(), command(Version::new(26, 0, 1)))
            .await;

        assert!(matches!(outcome, Err(CoreError::DeploymentNotFound { .. })));
        assert!(writes.lock().expect("not poisoned").is_empty());
    }

    /// The third operation the lock covers. It is refused by the settled
    /// check rather than by a lock of its own, and a test says so here so a
    /// later loosening of that check cannot quietly allow two at once.
    #[tokio::test]
    async fn a_second_upgrade_is_refused_while_one_is_running() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let service = UpgradeServiceImpl::new(
            repository(
                Some(deployment(
                    DeploymentStatus::Upgrading,
                    Version::new(26, 0, 0),
                )),
                writes.clone(),
            ),
            StubReleases::holding(vec![release(
                Version::new(27, 0, 0),
                ReleaseStatus::Available,
            )]),
            StubRuns::empty(),
            organisations(),
            StubPolicy::allowing(),
        );

        let outcome = service
            .request_upgrade(caller(), command(Version::new(27, 0, 0)))
            .await;

        assert!(matches!(
            outcome,
            Err(CoreError::DeploymentNotUpgradable { .. })
        ));
        assert!(writes.lock().expect("not poisoned").is_empty());
    }

    /// The permission is checked before the deployment is read. A caller with
    /// no right to move this organisation's versions must not learn whether a
    /// given id exists, let alone what it runs.
    #[tokio::test]
    async fn a_caller_without_the_permission_is_refused_before_anything_is_read() {
        let mut mock = MockDeploymentRepository::new();
        mock.expect_get_by_id().times(0);
        mock.expect_update().times(0);

        let policy = StubPolicy::refusing();
        let service = UpgradeServiceImpl::new(
            mock,
            StubReleases::holding(vec![release(
                Version::new(26, 0, 1),
                ReleaseStatus::Available,
            )]),
            StubRuns::empty(),
            organisations(),
            policy.clone(),
        );

        let outcome = service
            .request_upgrade(caller(), command(Version::new(26, 0, 1)))
            .await;

        assert!(matches!(outcome, Err(CoreError::PermissionDenied { .. })));
        assert_eq!(
            *policy.asked.lock().expect("not poisoned"),
            1,
            "the policy was not consulted"
        );
    }

    /// And the check actually happens on the allowing path too, so removing it
    /// cannot pass unnoticed.
    #[tokio::test]
    async fn the_permission_is_consulted_on_every_request() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let policy = StubPolicy::allowing();
        let service = UpgradeServiceImpl::new(
            repository(
                Some(deployment(
                    DeploymentStatus::Successful,
                    Version::new(26, 0, 0),
                )),
                writes,
            ),
            StubReleases::holding(vec![release(
                Version::new(26, 0, 1),
                ReleaseStatus::Available,
            )]),
            StubRuns::empty(),
            organisations(),
            policy.clone(),
        );

        service
            .request_upgrade(caller(), command(Version::new(26, 0, 1)))
            .await
            .expect("allowed");

        assert_eq!(*policy.asked.lock().expect("not poisoned"), 1);
    }

    fn settings(auto: AutoUpgradePolicy) -> SetUpgradeSettingsCommand {
        settings_with(auto, None)
    }

    fn settings_with(
        auto: AutoUpgradePolicy,
        maintenance_window: Option<MaintenanceWindow>,
    ) -> SetUpgradeSettingsCommand {
        SetUpgradeSettingsCommand {
            organisation_id: OrganisationId(ORGANISATION),
            deployment_id: DeploymentId(DEPLOYMENT),
            auto_upgrade: auto,
            maintenance_window,
        }
    }

    fn window() -> MaintenanceWindow {
        MaintenanceWindow::new(
            Weekday::Sun,
            NaiveTime::parse_from_str("03:00", "%H:%M").expect("a valid time"),
            Duration::hours(2),
            Tz::Europe__Paris,
        )
        .expect("a valid window")
    }

    /// Runs a settings write against a deployment holding `from`, and hands
    /// back both sides of it.
    async fn applied(
        from: (AutoUpgradePolicy, Option<MaintenanceWindow>),
        command: SetUpgradeSettingsCommand,
    ) -> AppliedUpgradeSettings {
        let mut held = deployment(DeploymentStatus::Successful, Version::new(26, 0, 0));
        held.auto_upgrade = from.0;
        held.maintenance_window = from.1;

        let service = UpgradeServiceImpl::new(
            repository(Some(held), Arc::new(Mutex::new(Vec::new()))),
            StubReleases::holding(Vec::new()),
            StubRuns::empty(),
            organisations(),
            StubPolicy::allowing(),
        );

        service
            .apply_upgrade_settings(caller(), command)
            .await
            .expect("allowed")
    }

    #[tokio::test]
    async fn settings_are_written_back() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let service = UpgradeServiceImpl::new(
            repository(
                Some(deployment(
                    DeploymentStatus::Successful,
                    Version::new(26, 0, 0),
                )),
                writes.clone(),
            ),
            StubReleases::holding(Vec::new()),
            StubRuns::empty(),
            organisations(),
            StubPolicy::allowing(),
        );

        let updated = service
            .set_upgrade_settings(caller(), settings(AutoUpgradePolicy::Patch))
            .await
            .expect("allowed");

        assert_eq!(updated.auto_upgrade, AutoUpgradePolicy::Patch);
        assert_eq!(writes.lock().expect("not poisoned").len(), 1);
    }

    /// Setting a policy is what causes upgrades to happen later, so it cannot
    /// be the cheap way around the permission that governs causing one now.
    #[tokio::test]
    async fn setting_the_policy_needs_the_same_permission_as_upgrading() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let service = UpgradeServiceImpl::new(
            repository(
                Some(deployment(
                    DeploymentStatus::Successful,
                    Version::new(26, 0, 0),
                )),
                writes.clone(),
            ),
            StubReleases::holding(Vec::new()),
            StubRuns::empty(),
            organisations(),
            StubPolicy::refusing(),
        );

        let outcome = service
            .set_upgrade_settings(caller(), settings(AutoUpgradePolicy::PatchAndMinor))
            .await;

        assert!(matches!(outcome, Err(CoreError::PermissionDenied { .. })));
        assert!(writes.lock().expect("not poisoned").is_empty());
    }

    /// Settings belong to a deployment, and a deployment belongs to one
    /// organisation. Naming someone else's does not reach it.
    #[tokio::test]
    async fn another_organisations_deployment_is_not_found() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let service = UpgradeServiceImpl::new(
            repository(None, writes.clone()),
            StubReleases::holding(Vec::new()),
            StubRuns::empty(),
            organisations(),
            StubPolicy::allowing(),
        );

        let outcome = service
            .set_upgrade_settings(caller(), settings(AutoUpgradePolicy::Patch))
            .await;

        assert!(matches!(outcome, Err(CoreError::DeploymentNotFound { .. })));
        assert!(writes.lock().expect("not poisoned").is_empty());
    }

    /// The value the row held is gone the instant the update lands, so the
    /// write is the only place it can be kept.
    #[tokio::test]
    async fn a_settings_write_hands_back_the_value_it_overwrote() {
        let applied = applied(
            (AutoUpgradePolicy::Manual, None),
            settings(AutoUpgradePolicy::PatchAndMinor),
        )
        .await;

        assert_eq!(applied.before.auto_upgrade, AutoUpgradePolicy::Manual);
        assert_eq!(applied.after.auto_upgrade, AutoUpgradePolicy::PatchAndMinor);
    }

    #[tokio::test]
    async fn changing_the_policy_records_both_sides_of_it() {
        let applied = applied(
            (AutoUpgradePolicy::Manual, None),
            settings(AutoUpgradePolicy::Patch),
        )
        .await;

        let changes = applied.configuration_changes().expect("nothing sensitive");

        assert_eq!(changes.len(), 1, "{changes:?}");
        assert_eq!(
            changes[0].action,
            AuditAction("deployment.auto_upgrade.updated".to_string())
        );
        assert_eq!(
            changes[0].change.before(),
            &json!({"auto_upgrade": "manual"})
        );
        assert_eq!(changes[0].change.after(), &json!({"auto_upgrade": "patch"}));
    }

    #[tokio::test]
    async fn changing_the_window_records_both_sides_of_it() {
        let applied = applied(
            (AutoUpgradePolicy::Patch, None),
            settings_with(AutoUpgradePolicy::Patch, Some(window())),
        )
        .await;

        let changes = applied.configuration_changes().expect("nothing sensitive");

        assert_eq!(changes.len(), 1, "{changes:?}");
        assert_eq!(
            changes[0].action,
            AuditAction("deployment.maintenance_window.updated".to_string())
        );
        assert_eq!(
            changes[0].change.before(),
            &json!({"maintenance_window": null})
        );
        assert_eq!(
            changes[0].change.after(),
            &json!({
                "maintenance_window": {
                    "day": "Sun",
                    "start": "03:00",
                    "duration_minutes": 120,
                    "timezone": "Europe/Paris",
                }
            })
        );
    }

    /// Two settings travel in one command, and they are two decisions. A
    /// reader asking when automation was turned on must not have to find it
    /// folded into an entry about a window.
    #[tokio::test]
    async fn moving_both_settings_leaves_one_statement_each() {
        let applied = applied(
            (AutoUpgradePolicy::Manual, None),
            settings_with(AutoUpgradePolicy::Patch, Some(window())),
        )
        .await;

        let actions: Vec<String> = applied
            .configuration_changes()
            .expect("nothing sensitive")
            .into_iter()
            .map(|change| change.action.0)
            .collect();

        assert_eq!(
            actions,
            vec![
                "deployment.auto_upgrade.updated".to_string(),
                "deployment.maintenance_window.updated".to_string(),
            ]
        );
    }

    /// An entry saying a value went from `patch` to `patch` is noise in the
    /// one place where noise hides the entry somebody is looking for.
    #[tokio::test]
    async fn a_write_that_moves_nothing_leaves_nothing() {
        let applied = applied(
            (AutoUpgradePolicy::Patch, Some(window())),
            settings_with(AutoUpgradePolicy::Patch, Some(window())),
        )
        .await;

        assert!(
            applied
                .configuration_changes()
                .expect("nothing sensitive")
                .is_empty()
        );
    }

    /// The guard against the common accident: forwarding the deployment, or
    /// the window struct, verbatim. Every key is named on purpose, so a field
    /// added to either type later cannot reach the trail until somebody
    /// decides it belongs there.
    #[tokio::test]
    async fn a_statement_carries_only_the_fields_it_names() {
        let applied = applied(
            (AutoUpgradePolicy::Manual, None),
            settings_with(AutoUpgradePolicy::Patch, Some(window())),
        )
        .await;

        let changes = applied.configuration_changes().expect("nothing sensitive");

        let policy = changes[0].change.after().as_object().expect("an object");
        assert_eq!(policy.keys().collect::<Vec<_>>(), vec!["auto_upgrade"]);

        let window = changes[1].change.after().as_object().expect("an object");
        assert_eq!(
            window.keys().collect::<Vec<_>>(),
            vec!["maintenance_window"]
        );

        let mut described: Vec<&String> = window["maintenance_window"]
            .as_object()
            .expect("an object")
            .keys()
            .collect();
        described.sort();
        assert_eq!(
            described,
            vec!["day", "duration_minutes", "start", "timezone"]
        );
    }

    #[tokio::test]
    async fn approving_an_upgrade_records_the_version_on_each_side() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let service = UpgradeServiceImpl::new(
            repository(
                Some(deployment(
                    DeploymentStatus::Successful,
                    Version::new(26, 0, 0),
                )),
                writes,
            ),
            StubReleases::holding(vec![release(
                Version::new(27, 0, 0),
                ReleaseStatus::Available,
            )]),
            StubRuns::empty(),
            organisations(),
            StubPolicy::allowing(),
        );

        let accepted = service
            .request_upgrade(caller(), command(Version::new(27, 0, 0)))
            .await
            .expect("accepted");

        let approved = ConfigurationChange::upgrade_approved(&accepted).expect("nothing sensitive");

        assert_eq!(
            approved.action,
            AuditAction("deployment.upgrade.approved".to_string())
        );
        assert_eq!(approved.change.before(), &json!({"version": "26.0.0"}));
        assert_eq!(approved.change.after(), &json!({"version": "27.0.0"}));
    }

    /// The target the caller asked for, not the first stepping stone. A
    /// reader of the trail wants to know what was approved; which versions the
    /// platform passes through on the way is its own decision.
    #[tokio::test]
    async fn approving_a_stepped_upgrade_records_the_target_not_the_first_step() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let mut target = release(Version::new(28, 0, 0), ReleaseStatus::Available);
        target.steps_through = vec![Version::new(27, 0, 0)];

        let service = UpgradeServiceImpl::new(
            repository(
                Some(deployment(
                    DeploymentStatus::Successful,
                    Version::new(26, 0, 0),
                )),
                writes,
            ),
            StubReleases::holding(vec![
                release(Version::new(27, 0, 0), ReleaseStatus::Available),
                target,
            ]),
            StubRuns::empty(),
            organisations(),
            StubPolicy::allowing(),
        );

        let accepted = service
            .request_upgrade(caller(), command(Version::new(28, 0, 0)))
            .await
            .expect("accepted");

        let approved = ConfigurationChange::upgrade_approved(&accepted).expect("nothing sensitive");

        assert_eq!(accepted.first_step(), &Version::new(27, 0, 0));
        assert_eq!(approved.change.after(), &json!({"version": "28.0.0"}));
    }

    /// An upgrade under way towards `steps.last()`, having started at `from`.
    fn open_run(from: Version, steps: Vec<Version>) -> UpgradeRun {
        UpgradeRun {
            id: UpgradeRunId(Uuid::from_u128(7)),
            deployment_id: DeploymentId(DEPLOYMENT),
            from_version: from,
            to_version: steps.last().expect("a path has an end").clone(),
            steps,
            change: VersionChange::Major,
            trigger: UpgradeTrigger::Scheduled,
            started_at: Utc::now(),
            outcome: None,
            detail: None,
            ended_at: None,
        }
    }

    fn advancing(
        deployment: Deployment,
        runs: StubRuns,
        writes: Arc<Mutex<Vec<Deployment>>>,
    ) -> UpgradeServiceImpl<
        MockDeploymentRepository,
        StubReleases,
        StubRuns,
        MockOrganisationRepository,
        StubPolicy,
    > {
        UpgradeServiceImpl::new(
            repository(Some(deployment), writes),
            StubReleases::holding(vec![]),
            runs,
            organisations(),
            StubPolicy::allowing(),
        )
    }

    /// The reason the path exists. A deployment three releases behind lands on
    /// the first stepping stone, and the next one has to follow.
    #[tokio::test]
    async fn a_step_that_landed_hands_back_the_next_one() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let runs = StubRuns::holding(vec![open_run(
            Version::new(25, 0, 0),
            vec![Version::new(26, 0, 0), Version::new(27, 0, 0)],
        )]);
        let service = advancing(
            deployment(DeploymentStatus::Successful, Version::new(26, 0, 0)),
            runs.clone(),
            writes.clone(),
        );

        let progress = service
            .advance_upgrade(DeploymentId(DEPLOYMENT))
            .await
            .expect("advanced");

        assert!(
            matches!(&progress, UpgradeProgress::NextStep { to, .. } if to == &Version::new(27, 0, 0)),
            "{progress:?}"
        );
        // Back to upgrading, or the next report would be read as a settled
        // deployment landing on a version nobody asked for.
        let written = writes.lock().expect("not poisoned");
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].status, DeploymentStatus::Upgrading);
        assert!(!runs.only().is_concluded());
    }

    #[tokio::test]
    async fn the_last_step_concludes_the_run() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let runs = StubRuns::holding(vec![open_run(
            Version::new(25, 0, 0),
            vec![Version::new(26, 0, 0), Version::new(27, 0, 0)],
        )]);
        let service = advancing(
            deployment(DeploymentStatus::Successful, Version::new(27, 0, 0)),
            runs.clone(),
            writes.clone(),
        );

        let progress = service
            .advance_upgrade(DeploymentId(DEPLOYMENT))
            .await
            .expect("advanced");

        assert_eq!(progress, UpgradeProgress::Arrived);
        assert!(runs.only().is_concluded());
        assert!(
            writes.lock().expect("not poisoned").is_empty(),
            "a deployment that arrived is already where it should be"
        );
    }

    /// Applying the next version to an instance that just came back unwell
    /// turns one bad version into two.
    #[tokio::test]
    async fn a_failed_deployment_is_not_pushed_any_further() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let runs = StubRuns::holding(vec![open_run(
            Version::new(25, 0, 0),
            vec![Version::new(26, 0, 0), Version::new(27, 0, 0)],
        )]);
        let service = advancing(
            deployment(DeploymentStatus::Failed, Version::new(26, 0, 0)),
            runs.clone(),
            writes.clone(),
        );

        let progress = service
            .advance_upgrade(DeploymentId(DEPLOYMENT))
            .await
            .expect("advanced");

        assert_eq!(progress, UpgradeProgress::GaveUp);
        let run = runs.only();
        assert!(run.is_concluded());
        assert!(run.detail.expect("a reason").contains("26.0.0"));
    }

    /// A report arriving while the step is still being applied says nothing
    /// about it, and must not close the run or start the next step.
    #[tokio::test]
    async fn a_deployment_still_upgrading_is_left_alone() {
        let runs = StubRuns::holding(vec![open_run(
            Version::new(25, 0, 0),
            vec![Version::new(26, 0, 0), Version::new(27, 0, 0)],
        )]);
        let service = advancing(
            deployment(DeploymentStatus::Upgrading, Version::new(25, 0, 0)),
            runs.clone(),
            Arc::new(Mutex::new(Vec::new())),
        );

        let progress = service
            .advance_upgrade(DeploymentId(DEPLOYMENT))
            .await
            .expect("advanced");

        assert_eq!(progress, UpgradeProgress::Untouched);
        assert!(!runs.only().is_concluded());
    }

    #[tokio::test]
    async fn a_deployment_with_no_upgrade_under_way_is_untouched() {
        let service = advancing(
            deployment(DeploymentStatus::Successful, Version::new(26, 0, 0)),
            StubRuns::empty(),
            Arc::new(Mutex::new(Vec::new())),
        );

        let progress = service
            .advance_upgrade(DeploymentId(DEPLOYMENT))
            .await
            .expect("advanced");

        assert_eq!(progress, UpgradeProgress::Untouched);
    }

    /// A run already concluded is history. Reading it back as the current
    /// attempt would restart an upgrade that ended days ago.
    #[tokio::test]
    async fn a_concluded_run_is_not_picked_up_again() {
        let mut done = open_run(Version::new(25, 0, 0), vec![Version::new(26, 0, 0)]);
        done.succeed(Utc::now()).expect("concluded");

        let service = advancing(
            deployment(DeploymentStatus::Successful, Version::new(25, 0, 0)),
            StubRuns::holding(vec![done]),
            Arc::new(Mutex::new(Vec::new())),
        );

        let progress = service
            .advance_upgrade(DeploymentId(DEPLOYMENT))
            .await
            .expect("advanced");

        assert_eq!(progress, UpgradeProgress::Untouched);
    }
}
