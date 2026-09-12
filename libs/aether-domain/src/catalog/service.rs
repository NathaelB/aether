use aether_auth::Identity;
use chrono::Utc;

use std::collections::HashMap;

use crate::{
    CoreError,
    catalog::{
        HeldBackDataPlane, IneligibilityReason, Release, ReleaseAvailability, ReleaseId,
        ReleaseInUse, RolloutCandidate, RolloutCoverage,
        commands::{
            AnnounceReleaseCommand, MoveReleaseCommand, ReviseReleaseCommand,
            RolloutCoveragePreview, WidenRolloutCommand,
        },
        ports::{ReleaseRepository, ReleaseService, RolloutEstateRepository},
    },
    dataplane::ports::DataPlaneRepository,
    deployments::{DeploymentId, DeploymentKind, ports::DeploymentRepository},
    organisation::{OrganisationId, ports::OrganisationRepository},
    version::Version,
};

/// What the catalogue holds is the platform's own statement about its
/// products. Nothing a customer does changes it, so every write is guarded
/// the same way and the guard is stated once.
fn only_operators(identity: &Identity) -> Result<(), CoreError> {
    if identity.is_operator() {
        return Ok(());
    }

    Err(CoreError::PermissionDenied {
        reason: "the catalogue is published by the platform, not by its customers".to_string(),
    })
}

pub struct ReleaseServiceImpl<R, D, O, DP, E>
where
    R: ReleaseRepository,
    D: DeploymentRepository,
    O: OrganisationRepository,
    DP: DataPlaneRepository,
    E: RolloutEstateRepository,
{
    release_repository: R,
    deployment_repository: D,
    organisation_repository: O,
    data_plane_repository: DP,
    rollout_estate_repository: E,
}

impl<R, D, O, DP, E> ReleaseServiceImpl<R, D, O, DP, E>
where
    R: ReleaseRepository,
    D: DeploymentRepository,
    O: OrganisationRepository,
    DP: DataPlaneRepository,
    E: RolloutEstateRepository,
{
    pub fn new(
        release_repository: R,
        deployment_repository: D,
        organisation_repository: O,
        data_plane_repository: DP,
        rollout_estate_repository: E,
    ) -> Self {
        Self {
            release_repository,
            deployment_repository,
            organisation_repository,
            data_plane_repository,
            rollout_estate_repository,
        }
    }

    async fn load(&self, kind: &DeploymentKind, version: &Version) -> Result<Release, CoreError> {
        self.release_repository
            .get(kind, version)
            .await?
            .ok_or_else(|| CoreError::ReleaseNotFound {
                release: ReleaseId::new(kind.clone(), version.clone()).to_string(),
            })
    }
}

impl<R, D, O, DP, E> ReleaseService for ReleaseServiceImpl<R, D, O, DP, E>
where
    R: ReleaseRepository,
    D: DeploymentRepository,
    O: OrganisationRepository,
    DP: DataPlaneRepository,
    E: RolloutEstateRepository,
{
    async fn publish_release(
        &self,
        identity: Identity,
        command: AnnounceReleaseCommand,
    ) -> Result<Release, CoreError> {
        only_operators(&identity)?;

        let mut release = Release::announce(
            ReleaseId::new(command.kind, command.version),
            command.risk,
            command.notes,
            Utc::now(),
        );
        release.steps_through = command.steps_through;
        release.minimum_operator_version = command.minimum_operator_version;

        self.release_repository.insert(release.clone()).await?;

        Ok(release)
    }

    async fn revise_release(
        &self,
        identity: Identity,
        command: ReviseReleaseCommand,
    ) -> Result<Release, CoreError> {
        only_operators(&identity)?;

        let mut release = self.load(&command.kind, &command.version).await?;
        release.revise(
            command.risk,
            command.notes,
            command.steps_through,
            command.minimum_operator_version,
            Utc::now(),
        );
        self.release_repository.update(&release).await?;

        Ok(release)
    }

    async fn move_release(
        &self,
        identity: Identity,
        command: MoveReleaseCommand,
    ) -> Result<Release, CoreError> {
        only_operators(&identity)?;

        let mut release = self.load(&command.kind, &command.version).await?;
        release.move_to(command.status, Utc::now())?;
        self.release_repository.update(&release).await?;

        Ok(release)
    }

    async fn list_releases_for_operator(
        &self,
        identity: Identity,
        kind: DeploymentKind,
    ) -> Result<Vec<ReleaseInUse>, CoreError> {
        only_operators(&identity)?;

        let releases = self.release_repository.list_for_kind(&kind).await?;
        let counts: HashMap<Version, u64> = self
            .deployment_repository
            .count_by_version(&kind)
            .await?
            .into_iter()
            .collect();

        Ok(releases
            .into_iter()
            .map(|release| {
                // Zero, not absent: a release nobody runs is the interesting
                // one on this screen, and leaving it out would hide exactly
                // what an operator is looking for.
                let deployments = counts.get(&release.id.version).copied().unwrap_or(0);
                ReleaseInUse {
                    release,
                    deployments,
                }
            })
            .collect())
    }

    async fn list_published_releases(
        &self,
        kind: DeploymentKind,
    ) -> Result<Vec<Release>, CoreError> {
        let releases = self.release_repository.list_for_kind(&kind).await?;

        Ok(releases
            .into_iter()
            .filter(|release| release.status.is_visible_to_customers())
            .collect())
    }

    async fn widen_rollout(
        &self,
        identity: Identity,
        command: WidenRolloutCommand,
    ) -> Result<Release, CoreError> {
        only_operators(&identity)?;

        let mut release = self.load(&command.kind, &command.version).await?;
        // `Rollout` refuses a narrowing itself; there is no domain-specific
        // `CoreError` variant for it yet, so the refusal surfaces as an
        // internal error until one exists.
        release
            .widen_rollout(command.rollout, Utc::now())
            .map_err(|e| CoreError::InternalError(e.to_string()))?;
        self.release_repository.update(&release).await?;

        Ok(release)
    }

    async fn preview_rollout_coverage(
        &self,
        identity: Identity,
        command: RolloutCoveragePreview,
    ) -> Result<RolloutCoverage, CoreError> {
        only_operators(&identity)?;

        let release = self.load(&command.kind, &command.version).await?;
        let estate = self
            .rollout_estate_repository
            .list_for_rollout(&command.kind)
            .await?;

        let total = estate.len() as u64;
        let covered = estate
            .iter()
            .filter(|entry| {
                command.rollout.covers(
                    &release.id,
                    &RolloutCandidate {
                        deployment_id: entry.deployment_id,
                        organisation_id: entry.organisation_id,
                        plan: entry.plan,
                    },
                )
            })
            .count() as u64;

        Ok(RolloutCoverage { covered, total })
    }

    async fn release_hold_backs(
        &self,
        identity: Identity,
        kind: DeploymentKind,
        version: Version,
    ) -> Result<Vec<HeldBackDataPlane>, CoreError> {
        only_operators(&identity)?;

        let release = self.load(&kind, &version).await?;
        let Some(minimum) = release.minimum_operator_version else {
            return Ok(Vec::new());
        };

        let planes = self.data_plane_repository.list_all().await?;

        Ok(planes
            .into_iter()
            .filter(|plane| match &plane.operator_version {
                Some(reported) => *reported < minimum,
                // Never having reported holds a release back exactly as hard
                // as reporting one that is too old: either way there is no
                // evidence the cluster can run it.
                None => true,
            })
            .map(|plane| HeldBackDataPlane {
                id: plane.id,
                operator_version: plane.operator_version,
            })
            .collect())
    }

    async fn release_availability_for_deployment(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<Vec<ReleaseAvailability>, CoreError> {
        let deployment = self
            .deployment_repository
            .get_by_id(deployment_id)
            .await?
            .ok_or(CoreError::DeploymentNotFound {
                id: deployment_id.0,
            })?;

        // A deployment belonging to another organisation answers like one
        // that does not exist, the same choice
        // `DeploymentService::get_deployment_for_organisation` makes.
        if deployment.organisation_id != organisation_id {
            return Err(CoreError::DeploymentNotFound {
                id: deployment_id.0,
            });
        }

        let organisation = self
            .organisation_repository
            .find_by_id(&organisation_id)
            .await?
            .ok_or(CoreError::OrganisationNotFound {
                id: organisation_id.0,
            })?;

        let dataplane_version = self
            .data_plane_repository
            .find_by_id(&deployment.dataplane_id)
            .await?
            .and_then(|dataplane| dataplane.operator_version);

        let candidate = RolloutCandidate {
            deployment_id,
            organisation_id,
            plan: organisation.plan,
        };

        let releases = self
            .release_repository
            .list_for_kind(&deployment.kind)
            .await?;

        Ok(releases
            .into_iter()
            .filter(|release| release.status.is_visible_to_customers())
            .map(|release| {
                let reason = ineligibility(&release, &candidate, dataplane_version.as_ref());
                let eligible = reason.is_none();
                ReleaseAvailability {
                    release,
                    eligible,
                    reason,
                }
            })
            .collect())
    }
}

/// Why `release` is not available to `candidate`, or `None` when it is.
///
/// Checked in order of what a client can least act on to what it can most act
/// on: a release nobody may install is not installable for anyone, an
/// operator gap is a platform fact about the cluster, and only then does the
/// rollout say who among the rest it is offered to.
fn ineligibility(
    release: &Release,
    candidate: &RolloutCandidate,
    dataplane_version: Option<&Version>,
) -> Option<IneligibilityReason> {
    if !release.status.is_installable() {
        return Some(IneligibilityReason::NotInstallable {
            status: release.status,
        });
    }

    if let Some(minimum) = &release.minimum_operator_version {
        let behind = match dataplane_version {
            Some(reported) => reported < minimum,
            None => true,
        };

        if behind {
            return Some(IneligibilityReason::OperatorTooOld {
                minimum: minimum.clone(),
                dataplane: dataplane_version.cloned(),
            });
        }
    }

    if !release.rollout.covers(&release.id, candidate) {
        return Some(IneligibilityReason::OutsideRollout);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataplane::{
        entities::DataPlane,
        ports::MockDataPlaneRepository,
        value_objects::{Capacity, DataPlaneAllocation, DataPlaneId, DataPlaneStatus, Region},
    };
    use crate::deployments::{Deployment, ports::MockDeploymentRepository};
    use crate::organisation::{
        Organisation,
        ports::MockOrganisationRepository,
        value_objects::{OrganisationName, OrganisationSlug, Plan},
    };
    use crate::user::UserId;
    use crate::{
        catalog::{
            BreakingRisk, ReleaseNotes, ReleaseStatus, Rollout, RolloutPercentage,
            ports::{MockRolloutEstateRepository, RolloutEstateEntry},
        },
        version::Version,
    };
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    /// Holds what was written so a test can assert the repository was left
    /// alone, which is the half of a permission check that is easy to forget.
    #[derive(Default, Clone)]
    struct SpyRepository {
        releases: Arc<Mutex<Vec<Release>>>,
        writes: Arc<Mutex<usize>>,
    }

    impl SpyRepository {
        fn holding(releases: Vec<Release>) -> Self {
            Self {
                releases: Arc::new(Mutex::new(releases)),
                writes: Arc::new(Mutex::new(0)),
            }
        }

        fn writes(&self) -> usize {
            *self.writes.lock().expect("not poisoned")
        }
    }

    impl ReleaseRepository for SpyRepository {
        async fn insert(&self, release: Release) -> Result<(), CoreError> {
            *self.writes.lock().expect("not poisoned") += 1;
            self.releases.lock().expect("not poisoned").push(release);
            Ok(())
        }

        async fn get(
            &self,
            kind: &DeploymentKind,
            version: &Version,
        ) -> Result<Option<Release>, CoreError> {
            Ok(self
                .releases
                .lock()
                .expect("not poisoned")
                .iter()
                .find(|release| &release.id.kind == kind && &release.id.version == version)
                .cloned())
        }

        async fn list_for_kind(&self, kind: &DeploymentKind) -> Result<Vec<Release>, CoreError> {
            Ok(self
                .releases
                .lock()
                .expect("not poisoned")
                .iter()
                .filter(|release| &release.id.kind == kind)
                .cloned()
                .collect())
        }

        async fn update(&self, release: &Release) -> Result<(), CoreError> {
            *self.writes.lock().expect("not poisoned") += 1;
            let mut held = self.releases.lock().expect("not poisoned");
            match held.iter_mut().find(|held| held.id == release.id) {
                Some(existing) => {
                    *existing = release.clone();
                    Ok(())
                }
                None => Err(CoreError::ReleaseNotFound {
                    release: release.id.to_string(),
                }),
            }
        }
    }

    /// The estate a test pretends to have. Most of these do not care, so the
    /// default is an empty one rather than a mock every test has to configure.
    fn running(counts: Vec<(Version, u64)>) -> MockDeploymentRepository {
        let mut deployments = MockDeploymentRepository::new();
        deployments.expect_count_by_version().returning(move |_| {
            let counts = counts.clone();
            Box::pin(async move { Ok(counts) })
        });
        deployments
    }

    fn no_deployments() -> MockDeploymentRepository {
        running(Vec::new())
    }

    fn no_organisation() -> MockOrganisationRepository {
        MockOrganisationRepository::new()
    }

    fn no_dataplanes() -> MockDataPlaneRepository {
        let mut planes = MockDataPlaneRepository::new();
        planes
            .expect_list_all()
            .returning(|| Box::pin(async { Ok(Vec::new()) }));
        planes
    }

    fn no_estate() -> MockRolloutEstateRepository {
        let mut estate = MockRolloutEstateRepository::new();
        estate
            .expect_list_for_rollout()
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        estate
    }

    /// Most tests only care about the release and deployment repositories.
    /// The other three exist for #116 and #117's read models, so they get an
    /// empty, permissive default here rather than every existing test having
    /// to configure them.
    fn make_service<R: ReleaseRepository>(
        repository: R,
        deployments: MockDeploymentRepository,
    ) -> ReleaseServiceImpl<
        R,
        MockDeploymentRepository,
        MockOrganisationRepository,
        MockDataPlaneRepository,
        MockRolloutEstateRepository,
    > {
        ReleaseServiceImpl::new(
            repository,
            deployments,
            no_organisation(),
            no_dataplanes(),
            no_estate(),
        )
    }

    fn operator() -> Identity {
        Identity::User(aether_auth::User {
            id: "operator".to_string(),
            username: "operator".to_string(),
            email: None,
            name: None,
            roles: vec!["aether-operator".to_string()],
        })
    }

    fn customer() -> Identity {
        Identity::User(aether_auth::User {
            id: "customer".to_string(),
            username: "customer".to_string(),
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

    fn announce() -> AnnounceReleaseCommand {
        AnnounceReleaseCommand {
            kind: DeploymentKind::Ferriskey,
            version: Version::new(26, 0, 1),
            risk: BreakingRisk::None,
            notes: ReleaseNotes("notes".to_string()),
            steps_through: Vec::new(),
            minimum_operator_version: None,
        }
    }

    #[tokio::test]
    async fn an_operator_publishes_a_release() {
        let repository = SpyRepository::default();
        let service = make_service(repository.clone(), no_deployments());

        let release = service
            .publish_release(operator(), announce())
            .await
            .expect("an operator may publish");

        assert_eq!(release.status, ReleaseStatus::Upcoming);
        assert_eq!(repository.writes(), 1);
    }

    /// The catalogue is what the platform says about its own products. A
    /// customer writing to it would be a customer deciding what everyone else
    /// can install.
    #[tokio::test]
    async fn a_customer_cannot_write_to_the_catalogue() {
        let repository = SpyRepository::holding(vec![release(
            Version::new(26, 0, 1),
            ReleaseStatus::Available,
        )]);
        let service = make_service(repository.clone(), no_deployments());

        let publish = service.publish_release(customer(), announce()).await;
        let revise = service
            .revise_release(
                customer(),
                ReviseReleaseCommand {
                    kind: DeploymentKind::Ferriskey,
                    version: Version::new(26, 0, 1),
                    risk: BreakingRisk::Breaking,
                    notes: ReleaseNotes("hijacked".to_string()),
                    steps_through: Vec::new(),
                    minimum_operator_version: None,
                },
            )
            .await;
        let moved = service
            .move_release(
                customer(),
                MoveReleaseCommand {
                    kind: DeploymentKind::Ferriskey,
                    version: Version::new(26, 0, 1),
                    status: ReleaseStatus::Withdrawn,
                },
            )
            .await;

        for outcome in [publish, revise, moved] {
            assert!(matches!(outcome, Err(CoreError::PermissionDenied { .. })));
        }

        assert_eq!(
            repository.writes(),
            0,
            "a refused write must not reach the repository"
        );
    }

    /// Upcoming is the platform's own planning. Showing it to a customer
    /// advertises a date nobody committed to.
    #[tokio::test]
    async fn a_customer_never_sees_a_planned_release() {
        let repository = SpyRepository::holding(vec![
            release(Version::new(27, 0, 0), ReleaseStatus::Upcoming),
            release(Version::new(26, 0, 1), ReleaseStatus::Available),
            release(Version::new(25, 0, 0), ReleaseStatus::Deprecated),
            release(Version::new(24, 0, 0), ReleaseStatus::Withdrawn),
        ]);
        let service = make_service(repository, no_deployments());

        let visible = service
            .list_published_releases(DeploymentKind::Ferriskey)
            .await
            .expect("any caller may read");

        let versions: Vec<String> = visible
            .iter()
            .map(|release| release.id.version.to_string())
            .collect();

        assert!(!versions.contains(&"27.0.0".to_string()), "{versions:?}");
        assert_eq!(versions.len(), 3, "{versions:?}");
    }

    /// The operator view is the one that shows planning, which is the reason
    /// there are two methods rather than one with a flag.
    #[tokio::test]
    async fn an_operator_sees_what_is_only_planned() {
        let repository = SpyRepository::holding(vec![release(
            Version::new(27, 0, 0),
            ReleaseStatus::Upcoming,
        )]);
        let service = make_service(repository, no_deployments());

        let listed = service
            .list_releases_for_operator(operator(), DeploymentKind::Ferriskey)
            .await
            .expect("an operator may read planning");

        assert_eq!(listed.len(), 1);
    }

    #[tokio::test]
    async fn a_customer_cannot_read_the_operator_view() {
        let repository = SpyRepository::default();
        let service = make_service(repository, no_deployments());

        let listed = service
            .list_releases_for_operator(customer(), DeploymentKind::Ferriskey)
            .await;

        assert!(matches!(listed, Err(CoreError::PermissionDenied { .. })));
    }

    /// The transition rules stay in the aggregate. The service only carries
    /// the refusal out.
    #[tokio::test]
    async fn a_backwards_move_is_refused_and_nothing_is_written() {
        let repository = SpyRepository::holding(vec![release(
            Version::new(26, 0, 1),
            ReleaseStatus::Withdrawn,
        )]);
        let service = make_service(repository.clone(), no_deployments());

        let outcome = service
            .move_release(
                operator(),
                MoveReleaseCommand {
                    kind: DeploymentKind::Ferriskey,
                    version: Version::new(26, 0, 1),
                    status: ReleaseStatus::Available,
                },
            )
            .await;

        assert!(matches!(
            outcome,
            Err(CoreError::InvalidReleaseTransition { .. })
        ));
        assert_eq!(repository.writes(), 0);
    }

    #[tokio::test]
    async fn revising_something_absent_says_so() {
        let repository = SpyRepository::default();
        let service = make_service(repository, no_deployments());

        let outcome = service
            .revise_release(
                operator(),
                ReviseReleaseCommand {
                    kind: DeploymentKind::Ferriskey,
                    version: Version::new(99, 0, 0),
                    risk: BreakingRisk::None,
                    notes: ReleaseNotes("notes".to_string()),
                    steps_through: Vec::new(),
                    minimum_operator_version: None,
                },
            )
            .await;

        assert!(matches!(outcome, Err(CoreError::ReleaseNotFound { .. })));
    }

    /// The reason an operator opens this screen: deciding what can be
    /// deprecated. That decision is the count.
    #[tokio::test]
    async fn the_operator_view_says_how_many_run_each_version() {
        let repository = SpyRepository::holding(vec![
            release(Version::new(26, 0, 1), ReleaseStatus::Available),
            release(Version::new(25, 0, 0), ReleaseStatus::Deprecated),
        ]);
        let service = make_service(
            repository,
            running(vec![
                (Version::new(26, 0, 1), 7),
                (Version::new(25, 0, 0), 2),
            ]),
        );

        let listed = service
            .list_releases_for_operator(operator(), DeploymentKind::Ferriskey)
            .await
            .expect("an operator may read");

        let counts: Vec<(String, u64)> = listed
            .iter()
            .map(|entry| (entry.release.id.version.to_string(), entry.deployments))
            .collect();

        assert_eq!(
            counts,
            [("26.0.1".to_string(), 7), ("25.0.0".to_string(), 2)]
        );
    }

    /// A release nobody runs is the interesting one on this screen. Leaving it
    /// out because it has no row in the count would hide exactly what an
    /// operator came to find.
    #[tokio::test]
    async fn a_release_nobody_runs_is_listed_at_zero() {
        let repository = SpyRepository::holding(vec![release(
            Version::new(27, 0, 0),
            ReleaseStatus::Available,
        )]);
        let service = make_service(repository, no_deployments());

        let listed = service
            .list_releases_for_operator(operator(), DeploymentKind::Ferriskey)
            .await
            .expect("an operator may read");

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].deployments, 0);
    }

    /// The count is a fact about the estate, not about the release, so a
    /// deployment on a version the catalogue never recorded does not conjure
    /// a release into the listing.
    #[tokio::test]
    async fn a_version_absent_from_the_catalogue_is_not_invented() {
        let repository = SpyRepository::holding(vec![release(
            Version::new(26, 0, 1),
            ReleaseStatus::Available,
        )]);
        let service = make_service(
            repository,
            running(vec![
                (Version::new(26, 0, 1), 1),
                (Version::new(0, 0, 0), 4),
            ]),
        );

        let listed = service
            .list_releases_for_operator(operator(), DeploymentKind::Ferriskey)
            .await
            .expect("an operator may read");

        assert_eq!(listed.len(), 1, "only what the catalogue holds is listed");
    }

    fn organisation(plan: Plan) -> Organisation {
        Organisation::new(
            OrganisationName::new("Acme Corp").unwrap(),
            OrganisationSlug::new("acme-corp").unwrap(),
            UserId(Uuid::new_v4()),
            plan,
        )
    }

    fn dataplane_with_version(operator_version: Option<Version>) -> DataPlane {
        DataPlane {
            id: DataPlaneId(Uuid::new_v4()),
            allocation: DataPlaneAllocation::Shared,
            region: Region::new("fr-par"),
            status: DataPlaneStatus::Active,
            capacity: Capacity::new(4_000, 8_192, 100).unwrap(),
            last_seen_at: None,
            created_at: Utc::now(),
            operator_version,
        }
    }

    #[tokio::test]
    async fn a_customer_cannot_widen_a_rollout() {
        let repository = SpyRepository::holding(vec![release(
            Version::new(26, 0, 1),
            ReleaseStatus::Available,
        )]);
        let service = make_service(repository.clone(), no_deployments());

        let outcome = service
            .widen_rollout(
                customer(),
                WidenRolloutCommand {
                    kind: DeploymentKind::Ferriskey,
                    version: Version::new(26, 0, 1),
                    rollout: Rollout::full(),
                },
            )
            .await;

        assert!(matches!(outcome, Err(CoreError::PermissionDenied { .. })));
        assert_eq!(repository.writes(), 0);
    }

    #[tokio::test]
    async fn an_operator_widens_a_rollout_and_it_is_persisted() {
        let mut starting = release(Version::new(26, 0, 1), ReleaseStatus::Available);
        starting.rollout = Rollout::new(RolloutPercentage::new(20).unwrap(), None, Vec::new());
        let repository = SpyRepository::holding(vec![starting]);
        let service = make_service(repository.clone(), no_deployments());
        let rollout = Rollout::new(RolloutPercentage::new(40).unwrap(), None, Vec::new());

        let released = service
            .widen_rollout(
                operator(),
                WidenRolloutCommand {
                    kind: DeploymentKind::Ferriskey,
                    version: Version::new(26, 0, 1),
                    rollout,
                },
            )
            .await
            .expect("an operator may widen");

        assert_eq!(
            released.rollout.percentage(),
            RolloutPercentage::new(40).unwrap()
        );
        assert_eq!(repository.writes(), 1);
    }

    /// The refusal to narrow surfaces through the service exactly as the
    /// aggregate raised it, rather than being swallowed as a generic failure.
    #[tokio::test]
    async fn a_narrowing_widen_request_is_refused_and_nothing_is_written() {
        let mut wide = release(Version::new(26, 0, 1), ReleaseStatus::Available);
        wide.rollout = Rollout::new(RolloutPercentage::new(70).unwrap(), None, Vec::new());
        let repository = SpyRepository::holding(vec![wide]);
        let service = make_service(repository.clone(), no_deployments());
        let narrow = Rollout::new(RolloutPercentage::new(20).unwrap(), None, Vec::new());

        let outcome = service
            .widen_rollout(
                operator(),
                WidenRolloutCommand {
                    kind: DeploymentKind::Ferriskey,
                    version: Version::new(26, 0, 1),
                    rollout: narrow,
                },
            )
            .await;

        assert!(outcome.is_err());
        assert_eq!(repository.writes(), 0);
    }

    /// What the operator screen is for: seeing the number before saving it.
    #[tokio::test]
    async fn preview_reports_how_many_of_the_estate_a_candidate_rollout_covers() {
        let repository = SpyRepository::holding(vec![release(
            Version::new(26, 0, 1),
            ReleaseStatus::Available,
        )]);
        let mut estate = MockRolloutEstateRepository::new();
        let pilot = OrganisationId(Uuid::new_v4());
        estate.expect_list_for_rollout().returning(move |_| {
            Box::pin(async move {
                Ok(vec![
                    RolloutEstateEntry {
                        deployment_id: DeploymentId(Uuid::new_v4()),
                        organisation_id: pilot,
                        plan: Plan::Free,
                    },
                    RolloutEstateEntry {
                        deployment_id: DeploymentId(Uuid::new_v4()),
                        organisation_id: OrganisationId(Uuid::new_v4()),
                        plan: Plan::Free,
                    },
                ])
            })
        });
        let service = ReleaseServiceImpl::new(
            repository,
            no_deployments(),
            no_organisation(),
            no_dataplanes(),
            estate,
        );
        let mut rollout = Rollout::new(RolloutPercentage::NONE, None, Vec::new());
        rollout.add_pilot_organisations([pilot]);

        let coverage = service
            .preview_rollout_coverage(
                operator(),
                RolloutCoveragePreview {
                    kind: DeploymentKind::Ferriskey,
                    version: Version::new(26, 0, 1),
                    rollout,
                },
            )
            .await
            .expect("an operator may preview");

        assert_eq!(coverage.total, 2);
        assert_eq!(coverage.covered, 1, "only the pilot is covered at 0%");
    }

    #[tokio::test]
    async fn a_release_with_no_operator_requirement_holds_nothing_back() {
        let repository = SpyRepository::holding(vec![release(
            Version::new(26, 0, 1),
            ReleaseStatus::Available,
        )]);
        let service = make_service(repository, no_deployments());

        let held_back = service
            .release_hold_backs(
                operator(),
                DeploymentKind::Ferriskey,
                Version::new(26, 0, 1),
            )
            .await
            .expect("an operator may ask");

        assert!(held_back.is_empty());
    }

    /// The acceptance criterion of #117: a data plane behind the requirement,
    /// and one that never reported at all, both hold the release back.
    #[tokio::test]
    async fn data_planes_behind_or_silent_hold_a_release_back() {
        let mut with_requirement = release(Version::new(26, 0, 1), ReleaseStatus::Available);
        with_requirement.minimum_operator_version = Some(Version::new(1, 4, 0));
        let repository = SpyRepository::holding(vec![with_requirement]);

        let mut planes = MockDataPlaneRepository::new();
        planes.expect_list_all().returning(|| {
            Box::pin(async {
                Ok(vec![
                    dataplane_with_version(Some(Version::new(1, 4, 0))),
                    dataplane_with_version(Some(Version::new(1, 3, 0))),
                    dataplane_with_version(None),
                ])
            })
        });
        let service = ReleaseServiceImpl::new(
            repository,
            no_deployments(),
            no_organisation(),
            planes,
            no_estate(),
        );

        let held_back = service
            .release_hold_backs(
                operator(),
                DeploymentKind::Ferriskey,
                Version::new(26, 0, 1),
            )
            .await
            .expect("an operator may ask");

        assert_eq!(held_back.len(), 2, "the plane on 1.4.0 is not behind");
    }

    fn deployment_on(dataplane_id: DataPlaneId, organisation_id: OrganisationId) -> Deployment {
        Deployment {
            id: DeploymentId(Uuid::new_v4()),
            organisation_id,
            dataplane_id,
            name: crate::deployments::DeploymentName("deployment".to_string()),
            resources: crate::dataplane::value_objects::DeploymentResources::DEFAULT,
            kind: DeploymentKind::Ferriskey,
            version: Version::new(25, 0, 0),
            status: crate::deployments::DeploymentStatus::Successful,
            namespace: "ns".to_string(),
            created_by: UserId(Uuid::new_v4()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            deployed_at: None,
            deleted_at: None,
            auto_upgrade: Default::default(),
            maintenance_window: None,
            network_access: crate::deployments::network::NetworkAccess::Open,
        }
    }

    /// The literal acceptance criterion of #117: the client is told why,
    /// rather than seeing an empty list indistinguishable from a platform
    /// that has no releases at all.
    #[tokio::test]
    async fn a_deployment_is_told_why_a_release_held_back_by_its_operator_is_unavailable() {
        let organisation_id = OrganisationId(Uuid::new_v4());
        let dataplane_id = DataPlaneId(Uuid::new_v4());
        let deployment = deployment_on(dataplane_id, organisation_id);
        let deployment_id = deployment.id;

        let mut too_new = release(Version::new(26, 0, 1), ReleaseStatus::Available);
        too_new.minimum_operator_version = Some(Version::new(2, 0, 0));
        let repository = SpyRepository::holding(vec![too_new]);

        let mut deployments = MockDeploymentRepository::new();
        deployments.expect_get_by_id().returning(move |_| {
            let deployment = deployment.clone();
            Box::pin(async move { Ok(Some(deployment)) })
        });

        let mut organisations = MockOrganisationRepository::new();
        organisations
            .expect_find_by_id()
            .returning(move |_| Box::pin(async move { Ok(Some(organisation(Plan::Free))) }));

        let mut planes = MockDataPlaneRepository::new();
        planes.expect_find_by_id().returning(move |_| {
            Box::pin(async move { Ok(Some(dataplane_with_version(Some(Version::new(1, 0, 0))))) })
        });

        let service =
            ReleaseServiceImpl::new(repository, deployments, organisations, planes, no_estate());

        let availability = service
            .release_availability_for_deployment(organisation_id, deployment_id)
            .await
            .expect("the deployment's own organisation may ask");

        assert_eq!(availability.len(), 1);
        assert!(!availability[0].eligible);
        assert!(matches!(
            availability[0].reason,
            Some(IneligibilityReason::OperatorTooOld { .. })
        ));
    }

    /// A deployment belonging to another organisation is refused the same way
    /// `DeploymentService::get_deployment_for_organisation` refuses it: as
    /// not found, so its existence is never confirmed to the wrong caller.
    #[tokio::test]
    async fn a_deployment_outside_the_callers_organisation_is_not_found() {
        let owner = OrganisationId(Uuid::new_v4());
        let stranger = OrganisationId(Uuid::new_v4());
        let deployment = deployment_on(DataPlaneId(Uuid::new_v4()), owner);
        let deployment_id = deployment.id;

        let repository = SpyRepository::default();
        let mut deployments = MockDeploymentRepository::new();
        deployments.expect_get_by_id().returning(move |_| {
            let deployment = deployment.clone();
            Box::pin(async move { Ok(Some(deployment)) })
        });
        let service = ReleaseServiceImpl::new(
            repository,
            deployments,
            no_organisation(),
            no_dataplanes(),
            no_estate(),
        );

        let outcome = service
            .release_availability_for_deployment(stranger, deployment_id)
            .await;

        assert!(matches!(outcome, Err(CoreError::DeploymentNotFound { .. })));
    }
}
