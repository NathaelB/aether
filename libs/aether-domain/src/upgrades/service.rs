use chrono::Utc;
use tracing::info;

use aether_auth::Identity;

use crate::{
    CoreError,
    catalog::ports::ReleaseRepository,
    deployments::{Deployment, DeploymentStatus, ports::DeploymentRepository},
    upgrades::{
        commands::{RequestUpgradeCommand, SetUpgradeSettingsCommand},
        ports::{AcceptedUpgrade, UpgradePolicy, UpgradeService},
    },
};

pub struct UpgradeServiceImpl<D, R, P>
where
    D: DeploymentRepository,
    R: ReleaseRepository,
    P: UpgradePolicy,
{
    deployment_repository: D,
    release_repository: R,
    policy: P,
}

impl<D, R, P> UpgradeServiceImpl<D, R, P>
where
    D: DeploymentRepository,
    R: ReleaseRepository,
    P: UpgradePolicy,
{
    pub fn new(deployment_repository: D, release_repository: R, policy: P) -> Self {
        Self {
            deployment_repository,
            release_repository,
            policy,
        }
    }
}

impl<D, R, P> UpgradeService for UpgradeServiceImpl<D, R, P>
where
    D: DeploymentRepository,
    R: ReleaseRepository,
    P: UpgradePolicy,
{
    async fn set_upgrade_settings(
        &self,
        identity: Identity,
        command: SetUpgradeSettingsCommand,
    ) -> Result<Deployment, CoreError> {
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

        deployment.auto_upgrade = command.auto_upgrade;
        deployment.maintenance_window = command.maintenance_window;
        deployment.updated_at = Utc::now();
        self.deployment_repository
            .update(deployment.clone())
            .await?;

        Ok(deployment)
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

        // Where the no downgrade rule lives: on the step, not on the version
        // type. On the type it would also forbid the rollback a failed upgrade
        // needs, which restores a version the deployment came from.
        let change = deployment.version.change_to(&command.target)?;

        info!(
            deployment_id = %deployment.id,
            from = %deployment.version,
            to = %command.target,
            change = ?change,
            "accepting an upgrade"
        );

        deployment.status = DeploymentStatus::Upgrading;
        deployment.updated_at = Utc::now();
        self.deployment_repository
            .update(deployment.clone())
            .await?;

        Ok(AcceptedUpgrade { deployment, change })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upgrades::policy::AutoUpgradePolicy;
    use crate::{
        catalog::{BreakingRisk, Release, ReleaseId, ReleaseNotes, ReleaseStatus},
        dataplane::value_objects::{DataPlaneId, DeploymentResources},
        deployments::{
            Deployment, DeploymentId, DeploymentKind, DeploymentName,
            ports::MockDeploymentRepository,
        },
        organisation::OrganisationId,
        user::UserId,
        version::{Version, VersionChange, VersionError},
    };
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

        async fn list_for_kind(&self, _kind: &DeploymentKind) -> Result<Vec<Release>, CoreError> {
            unreachable!("an upgrade asks for one release, not a listing")
        }

        async fn update(&self, _release: &Release) -> Result<(), CoreError> {
            unreachable!("an upgrade never writes to the catalogue")
        }
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

    fn release(version: Version, status: ReleaseStatus) -> Release {
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
            policy.clone(),
        );

        service
            .request_upgrade(caller(), command(Version::new(26, 0, 1)))
            .await
            .expect("allowed");

        assert_eq!(*policy.asked.lock().expect("not poisoned"), 1);
    }

    fn settings(auto: AutoUpgradePolicy) -> SetUpgradeSettingsCommand {
        SetUpgradeSettingsCommand {
            organisation_id: OrganisationId(ORGANISATION),
            deployment_id: DeploymentId(DEPLOYMENT),
            auto_upgrade: auto,
            maintenance_window: None,
        }
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
            StubPolicy::allowing(),
        );

        let outcome = service
            .set_upgrade_settings(caller(), settings(AutoUpgradePolicy::Patch))
            .await;

        assert!(matches!(outcome, Err(CoreError::DeploymentNotFound { .. })));
        assert!(writes.lock().expect("not poisoned").is_empty());
    }
}
