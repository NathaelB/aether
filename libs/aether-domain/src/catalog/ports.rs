use std::future::Future;

use crate::{CoreError, catalog::Release, deployments::DeploymentKind, version::Version};

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
