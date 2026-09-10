use std::future::Future;

use aether_auth::Identity;

use crate::{
    CoreError,
    catalog::{
        Release, ReleaseInUse,
        commands::{AnnounceReleaseCommand, MoveReleaseCommand, ReviseReleaseCommand},
    },
    deployments::DeploymentKind,
    version::Version,
};

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
}
