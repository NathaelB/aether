use aether_auth::Identity;
use chrono::Utc;

use std::collections::HashMap;

use crate::{
    CoreError,
    catalog::{
        Release, ReleaseId, ReleaseInUse,
        commands::{AnnounceReleaseCommand, MoveReleaseCommand, ReviseReleaseCommand},
        ports::{ReleaseRepository, ReleaseService},
    },
    deployments::{DeploymentKind, ports::DeploymentRepository},
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

pub struct ReleaseServiceImpl<R, D>
where
    R: ReleaseRepository,
    D: DeploymentRepository,
{
    release_repository: R,
    deployment_repository: D,
}

impl<R, D> ReleaseServiceImpl<R, D>
where
    R: ReleaseRepository,
    D: DeploymentRepository,
{
    pub fn new(release_repository: R, deployment_repository: D) -> Self {
        Self {
            release_repository,
            deployment_repository,
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

impl<R, D> ReleaseService for ReleaseServiceImpl<R, D>
where
    R: ReleaseRepository,
    D: DeploymentRepository,
{
    async fn publish_release(
        &self,
        identity: Identity,
        command: AnnounceReleaseCommand,
    ) -> Result<Release, CoreError> {
        only_operators(&identity)?;

        let release = Release::announce(
            ReleaseId::new(command.kind, command.version),
            command.risk,
            command.notes,
            Utc::now(),
        );

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
        release.revise(command.risk, command.notes, Utc::now());
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deployments::ports::MockDeploymentRepository;
    use crate::{
        catalog::{BreakingRisk, ReleaseNotes, ReleaseStatus},
        version::Version,
    };
    use std::sync::{Arc, Mutex};

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
        }
    }

    #[tokio::test]
    async fn an_operator_publishes_a_release() {
        let repository = SpyRepository::default();
        let service = ReleaseServiceImpl::new(repository.clone(), no_deployments());

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
        let service = ReleaseServiceImpl::new(repository.clone(), no_deployments());

        let publish = service.publish_release(customer(), announce()).await;
        let revise = service
            .revise_release(
                customer(),
                ReviseReleaseCommand {
                    kind: DeploymentKind::Ferriskey,
                    version: Version::new(26, 0, 1),
                    risk: BreakingRisk::Breaking,
                    notes: ReleaseNotes("hijacked".to_string()),
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
        let service = ReleaseServiceImpl::new(repository, no_deployments());

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
        let service = ReleaseServiceImpl::new(repository, no_deployments());

        let listed = service
            .list_releases_for_operator(operator(), DeploymentKind::Ferriskey)
            .await
            .expect("an operator may read planning");

        assert_eq!(listed.len(), 1);
    }

    #[tokio::test]
    async fn a_customer_cannot_read_the_operator_view() {
        let repository = SpyRepository::default();
        let service = ReleaseServiceImpl::new(repository, no_deployments());

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
        let service = ReleaseServiceImpl::new(repository.clone(), no_deployments());

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
        let service = ReleaseServiceImpl::new(repository, no_deployments());

        let outcome = service
            .revise_release(
                operator(),
                ReviseReleaseCommand {
                    kind: DeploymentKind::Ferriskey,
                    version: Version::new(99, 0, 0),
                    risk: BreakingRisk::None,
                    notes: ReleaseNotes("notes".to_string()),
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
        let service = ReleaseServiceImpl::new(
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
        let service = ReleaseServiceImpl::new(repository, no_deployments());

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
        let service = ReleaseServiceImpl::new(
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
}
