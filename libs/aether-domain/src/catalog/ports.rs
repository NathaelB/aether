use std::future::Future;

use aether_auth::Identity;

use crate::{
    CoreError,
    catalog::{
        HeldBackDataPlane, Release, ReleaseAvailability, ReleaseInUse, RolloutCoverage,
        commands::{
            AnnounceReleaseCommand, MoveReleaseCommand, ReviseReleaseCommand,
            RolloutCoveragePreview, WidenRolloutCommand,
        },
    },
    deployments::{DeploymentId, DeploymentKind},
    organisation::{OrganisationId, value_objects::Plan},
    version::Version,
};

/// What the operator screen needs about the estate to preview a rollout's
/// coverage: one deployment, the plan its organisation is on.
///
/// A dedicated read, not `DeploymentRepository` extended with a new method:
/// nothing else in the platform needs "every live deployment of a product,
/// with its organisation's plan" in bulk, and adding it to a port every other
/// bounded context already depends on would spend a general capability on one
/// screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RolloutEstateEntry {
    pub deployment_id: DeploymentId,
    pub organisation_id: OrganisationId,
    pub plan: Plan,
}

#[cfg_attr(test, mockall::automock)]
pub trait RolloutEstateRepository: Send + Sync {
    /// Every live deployment of a product, with the plan its organisation is
    /// on. A deployment being torn down is not included.
    fn list_for_rollout(
        &self,
        kind: &DeploymentKind,
    ) -> impl Future<Output = Result<Vec<RolloutEstateEntry>, CoreError>> + Send;
}

pub trait ReleaseRepository: Send + Sync {
    /// Records a release the catalogue does not already hold.
    ///
    /// Fails with [`CoreError::ReleaseAlreadyExists`] rather than overwriting.
    /// A second row for one release is two answers to "may this be
    /// installed", and silently replacing the first would lose whatever the
    /// notes said about it.
    fn insert(&self, release: Release) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn get(
        &self,
        kind: &DeploymentKind,
        version: &Version,
    ) -> impl Future<Output = Result<Option<Release>, CoreError>> + Send;

    /// Every release of a product, newest first.
    ///
    /// Unfiltered on purpose: the operator screen wants withdrawn releases and
    /// the customer view does not, and deciding that here would make the port
    /// answer a question that belongs to the caller.
    fn list_for_kind(
        &self,
        kind: &DeploymentKind,
    ) -> impl Future<Output = Result<Vec<Release>, CoreError>> + Send;

    /// Writes back status, risk and notes. The identity never moves.
    fn update(&self, release: &Release) -> impl Future<Output = Result<(), CoreError>> + Send;
}

/// What the catalogue offers, and to whom.
///
/// Writing is the platform's own act, so every write is operator only.
/// Reading is split in two rather than filtered by a flag: an operator sees
/// planning, a customer sees what exists. One method with a boolean would put
/// the decision at the call site, which is where it eventually gets it wrong.
pub trait ReleaseService: Send + Sync {
    fn publish_release(
        &self,
        identity: Identity,
        command: AnnounceReleaseCommand,
    ) -> impl Future<Output = Result<Release, CoreError>> + Send;

    fn revise_release(
        &self,
        identity: Identity,
        command: ReviseReleaseCommand,
    ) -> impl Future<Output = Result<Release, CoreError>> + Send;

    fn move_release(
        &self,
        identity: Identity,
        command: MoveReleaseCommand,
    ) -> impl Future<Output = Result<Release, CoreError>> + Send;

    /// Everything the catalogue holds for a product, planning included, with
    /// how many deployments run each version.
    ///
    /// The count travels with the listing rather than as its own endpoint:
    /// the only reason to read this view is to decide what to deprecate or
    /// withdraw, and that decision is the count.
    fn list_releases_for_operator(
        &self,
        identity: Identity,
        kind: DeploymentKind,
    ) -> impl Future<Output = Result<Vec<ReleaseInUse>, CoreError>> + Send;

    /// What a customer may see: everything except what has only been planned.
    ///
    /// Takes no identity because it grants nothing. Any authenticated caller
    /// gets the same answer, and the middleware has already decided whether
    /// there is a caller at all.
    fn list_published_releases(
        &self,
        kind: DeploymentKind,
    ) -> impl Future<Output = Result<Vec<Release>, CoreError>> + Send;

    /// Widens a release's rollout. Operator only, and refuses anything that
    /// would narrow it -- see the module docs on [`crate::catalog::Rollout`].
    fn widen_rollout(
        &self,
        identity: Identity,
        command: WidenRolloutCommand,
    ) -> impl Future<Output = Result<Release, CoreError>> + Send;

    /// How many deployments a candidate rollout would cover, without saving
    /// it. Operator only: this previews a change to the platform's own
    /// publishing decision, not something a customer asks about themselves.
    fn preview_rollout_coverage(
        &self,
        identity: Identity,
        command: RolloutCoveragePreview,
    ) -> impl Future<Output = Result<RolloutCoverage, CoreError>> + Send;

    /// Which data planes are behind the operator version this release
    /// requires. Operator only, and empty whenever the release has no
    /// requirement or nothing is behind it.
    fn release_hold_backs(
        &self,
        identity: Identity,
        kind: DeploymentKind,
        version: Version,
    ) -> impl Future<Output = Result<Vec<HeldBackDataPlane>, CoreError>> + Send;

    /// Every release of a product as one specific deployment sees it, with a
    /// reason attached to every one it may not install.
    ///
    /// Scoped by `organisation_id` the same way
    /// [`crate::deployments::ports::DeploymentService::get_deployment_for_organisation`]
    /// is: a deployment belonging to another organisation is reported as not
    /// found rather than confirming its existence to a caller with no
    /// business asking.
    fn release_availability_for_deployment(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> impl Future<Output = Result<Vec<ReleaseAvailability>, CoreError>> + Send;
}
