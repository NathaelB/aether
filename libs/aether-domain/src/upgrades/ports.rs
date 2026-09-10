use std::future::Future;

use crate::{
    CoreError, deployments::Deployment, upgrades::commands::RequestUpgradeCommand,
    version::VersionChange,
};

/// A deployment accepted for an upgrade, and what kind of step it is.
///
/// The classification travels with the decision because every later stage
/// needs it and none of them should work it out again: the client policy in
/// V4 reads it, and two places computing the same thing is two places to get
/// it wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedUpgrade {
    pub deployment: Deployment,
    pub change: VersionChange,
}

pub trait UpgradeService: Send + Sync {
    /// Accepts an upgrade and moves the deployment into `Upgrading`.
    ///
    /// Refuses rather than queues. A deployment that is not settled, a target
    /// the catalogue will not install, and a target that is not ahead are all
    /// answers the caller can act on now.
    fn request_upgrade(
        &self,
        command: RequestUpgradeCommand,
    ) -> impl Future<Output = Result<AcceptedUpgrade, CoreError>> + Send;
}
