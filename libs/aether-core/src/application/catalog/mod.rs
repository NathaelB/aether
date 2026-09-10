use aether_auth::Identity;
use aether_domain::{
    CoreError,
    catalog::{
        Release, ReleaseInUse,
        commands::{AnnounceReleaseCommand, MoveReleaseCommand, ReviseReleaseCommand},
        ports::ReleaseService,
        service::ReleaseServiceImpl,
    },
    deployments::DeploymentKind,
};
use aether_macros::transactional;

use crate::AetherService;

impl ReleaseService for AetherService {
    #[transactional(release, deployment)]
    async fn publish_release(
        &self,
        identity: Identity,
        command: AnnounceReleaseCommand,
    ) -> Result<Release, CoreError> {
        ReleaseServiceImpl::new(release_repository, deployment_repository)
            .publish_release(identity, command)
            .await
    }

    #[transactional(release, deployment)]
    async fn revise_release(
        &self,
        identity: Identity,
        command: ReviseReleaseCommand,
    ) -> Result<Release, CoreError> {
        ReleaseServiceImpl::new(release_repository, deployment_repository)
            .revise_release(identity, command)
            .await
    }

    #[transactional(release, deployment)]
    async fn move_release(
        &self,
        identity: Identity,
        command: MoveReleaseCommand,
    ) -> Result<Release, CoreError> {
        ReleaseServiceImpl::new(release_repository, deployment_repository)
            .move_release(identity, command)
            .await
    }

    #[transactional(release, deployment)]
    async fn list_releases_for_operator(
        &self,
        identity: Identity,
        kind: DeploymentKind,
    ) -> Result<Vec<ReleaseInUse>, CoreError> {
        ReleaseServiceImpl::new(release_repository, deployment_repository)
            .list_releases_for_operator(identity, kind)
            .await
    }

    #[transactional(release, deployment)]
    async fn list_published_releases(
        &self,
        kind: DeploymentKind,
    ) -> Result<Vec<Release>, CoreError> {
        ReleaseServiceImpl::new(release_repository, deployment_repository)
            .list_published_releases(kind)
            .await
    }
}
