use aether_auth::Identity;
use aether_domain::{
    CoreError,
    catalog::{
        HeldBackDataPlane, Release, ReleaseAvailability, ReleaseInUse, RolloutCoverage,
        commands::{
            AnnounceReleaseCommand, MoveReleaseCommand, ReviseReleaseCommand,
            RolloutCoveragePreview, WidenRolloutCommand,
        },
        ports::ReleaseService,
        service::ReleaseServiceImpl,
    },
    deployments::{DeploymentId, DeploymentKind},
    organisation::OrganisationId,
    version::Version,
};
use aether_macros::transactional;
use aether_postgres::catalog::PostgresRolloutEstateRepository;

use crate::AetherService;

impl ReleaseService for AetherService {
    #[transactional(release, deployment, organisation, data_plane)]
    async fn publish_release(
        &self,
        identity: Identity,
        command: AnnounceReleaseCommand,
    ) -> Result<Release, CoreError> {
        ReleaseServiceImpl::new(
            release_repository,
            deployment_repository,
            organisation_repository,
            data_plane_repository,
            PostgresRolloutEstateRepository::new(&tx),
        )
        .publish_release(identity, command)
        .await
    }

    #[transactional(release, deployment, organisation, data_plane)]
    async fn revise_release(
        &self,
        identity: Identity,
        command: ReviseReleaseCommand,
    ) -> Result<Release, CoreError> {
        ReleaseServiceImpl::new(
            release_repository,
            deployment_repository,
            organisation_repository,
            data_plane_repository,
            PostgresRolloutEstateRepository::new(&tx),
        )
        .revise_release(identity, command)
        .await
    }

    #[transactional(release, deployment, organisation, data_plane)]
    async fn move_release(
        &self,
        identity: Identity,
        command: MoveReleaseCommand,
    ) -> Result<Release, CoreError> {
        ReleaseServiceImpl::new(
            release_repository,
            deployment_repository,
            organisation_repository,
            data_plane_repository,
            PostgresRolloutEstateRepository::new(&tx),
        )
        .move_release(identity, command)
        .await
    }

    #[transactional(release, deployment, organisation, data_plane)]
    async fn list_releases_for_operator(
        &self,
        identity: Identity,
        kind: DeploymentKind,
    ) -> Result<Vec<ReleaseInUse>, CoreError> {
        ReleaseServiceImpl::new(
            release_repository,
            deployment_repository,
            organisation_repository,
            data_plane_repository,
            PostgresRolloutEstateRepository::new(&tx),
        )
        .list_releases_for_operator(identity, kind)
        .await
    }

    #[transactional(release, deployment, organisation, data_plane)]
    async fn list_published_releases(
        &self,
        kind: DeploymentKind,
    ) -> Result<Vec<Release>, CoreError> {
        ReleaseServiceImpl::new(
            release_repository,
            deployment_repository,
            organisation_repository,
            data_plane_repository,
            PostgresRolloutEstateRepository::new(&tx),
        )
        .list_published_releases(kind)
        .await
    }

    #[transactional(release, deployment, organisation, data_plane)]
    async fn widen_rollout(
        &self,
        identity: Identity,
        command: WidenRolloutCommand,
    ) -> Result<Release, CoreError> {
        ReleaseServiceImpl::new(
            release_repository,
            deployment_repository,
            organisation_repository,
            data_plane_repository,
            PostgresRolloutEstateRepository::new(&tx),
        )
        .widen_rollout(identity, command)
        .await
    }

    #[transactional(release, deployment, organisation, data_plane)]
    async fn preview_rollout_coverage(
        &self,
        identity: Identity,
        command: RolloutCoveragePreview,
    ) -> Result<RolloutCoverage, CoreError> {
        ReleaseServiceImpl::new(
            release_repository,
            deployment_repository,
            organisation_repository,
            data_plane_repository,
            PostgresRolloutEstateRepository::new(&tx),
        )
        .preview_rollout_coverage(identity, command)
        .await
    }

    #[transactional(release, deployment, organisation, data_plane)]
    async fn release_hold_backs(
        &self,
        identity: Identity,
        kind: DeploymentKind,
        version: Version,
    ) -> Result<Vec<HeldBackDataPlane>, CoreError> {
        ReleaseServiceImpl::new(
            release_repository,
            deployment_repository,
            organisation_repository,
            data_plane_repository,
            PostgresRolloutEstateRepository::new(&tx),
        )
        .release_hold_backs(identity, kind, version)
        .await
    }

    #[transactional(release, deployment, organisation, data_plane)]
    async fn release_availability_for_deployment(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<Vec<ReleaseAvailability>, CoreError> {
        ReleaseServiceImpl::new(
            release_repository,
            deployment_repository,
            organisation_repository,
            data_plane_repository,
            PostgresRolloutEstateRepository::new(&tx),
        )
        .release_availability_for_deployment(organisation_id, deployment_id)
        .await
    }
}
