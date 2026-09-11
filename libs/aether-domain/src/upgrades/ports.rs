use std::future::Future;

use aether_auth::Identity;

use crate::{
    CoreError,
    deployments::Deployment,
    organisation::OrganisationId,
    upgrades::commands::{RequestUpgradeCommand, SetUpgradeSettingsCommand},
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

/// Who may move a deployment to another version.
///
/// Separate from whatever governs changing its settings: the platform has no
/// downgrade, so an upgrade is a one way door and reads as one.
pub trait UpgradePolicy: Send + Sync {
    fn can_upgrade_deployment(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;
}

pub trait UpgradeService: Send + Sync {
    /// Accepts an upgrade and moves the deployment into `Upgrading`.
    ///
    /// Refuses rather than queues. A deployment that is not settled, a target
    /// the catalogue will not install, and a target that is not ahead are all
    /// answers the caller can act on now.
    fn request_upgrade(
        &self,
        identity: Identity,
        command: RequestUpgradeCommand,
    ) -> impl Future<Output = Result<AcceptedUpgrade, CoreError>> + Send;

    /// Records what the customer delegates, and when.
    ///
    /// Guarded by the same permission as triggering an upgrade. Setting a
    /// policy is what causes upgrades to happen later, so treating it as the
    /// lesser right would make it the way around the greater one.
    fn set_upgrade_settings(
        &self,
        identity: Identity,
        command: SetUpgradeSettingsCommand,
    ) -> impl Future<Output = Result<Deployment, CoreError>> + Send;
}
