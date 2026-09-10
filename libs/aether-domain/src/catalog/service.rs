use aether_auth::Identity;
use chrono::Utc;

use crate::{
    CoreError,
    catalog::{
        Release, ReleaseId,
        commands::{AnnounceReleaseCommand, MoveReleaseCommand, ReviseReleaseCommand},
        ports::{ReleaseRepository, ReleaseService},
    },
    deployments::DeploymentKind,
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

pub struct ReleaseServiceImpl<R>
where
    R: ReleaseRepository,
{
    release_repository: R,
}

impl<R> ReleaseServiceImpl<R>
where
    R: ReleaseRepository,
{
    pub fn new(release_repository: R) -> Self {
        Self { release_repository }
    }

    async fn load(
        &self,
        kind: &DeploymentKind,
        version: &crate::version::Version,
    ) -> Result<Release, CoreError> {
        self.release_repository
            .get(kind, version)
            .await?
            .ok_or_else(|| CoreError::ReleaseNotFound {
                release: ReleaseId::new(kind.clone(), version.clone()).to_string(),
            })
    }
}

impl<R> ReleaseService for ReleaseServiceImpl<R>
where
    R: ReleaseRepository,
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
    ) -> Result<Vec<Release>, CoreError> {
        only_operators(&identity)?;

        self.release_repository.list_for_kind(&kind).await
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
        let service = ReleaseServiceImpl::new(repository.clone());

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
        let service = ReleaseServiceImpl::new(repository.clone());

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
        let service = ReleaseServiceImpl::new(repository);

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
        let service = ReleaseServiceImpl::new(repository);

        let listed = service
            .list_releases_for_operator(operator(), DeploymentKind::Ferriskey)
            .await
            .expect("an operator may read planning");

        assert_eq!(listed.len(), 1);
    }

    #[tokio::test]
    async fn a_customer_cannot_read_the_operator_view() {
        let repository = SpyRepository::default();
        let service = ReleaseServiceImpl::new(repository);

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
        let service = ReleaseServiceImpl::new(repository.clone());

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
        let service = ReleaseServiceImpl::new(repository);

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
}
