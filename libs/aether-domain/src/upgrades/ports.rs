use std::future::Future;

use aether_auth::Identity;

use crate::{
    CoreError,
    deployments::{Deployment, DeploymentId},
    organisation::OrganisationId,
    upgrades::{
        commands::{RequestUpgradeCommand, SetUpgradeSettingsCommand},
        path::UpgradePath,
        run::InFlightUpgrade,
    },
    version::VersionChange,
};

/// A deployment accepted for an upgrade, what kind of step it is, and the
/// route it will take to get there.
///
/// The classification travels with the decision because every later stage
/// needs it and none of them should work it out again: the client policy in
/// V4 reads it, and two places computing the same thing is two places to get
/// it wrong. The path travels for the same reason and one more: it is decided
/// against the catalogue as it stood when the upgrade was accepted, and a
/// release withdrawn an hour later must not silently reroute an upgrade
/// already under way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedUpgrade {
    pub deployment: Deployment,
    pub change: VersionChange,
    pub path: UpgradePath,
}

impl AcceptedUpgrade {
    /// The version to apply first. Not the target: a deployment several
    /// releases behind passes through the ones in between, one at a time.
    pub fn first_step(&self) -> &crate::version::Version {
        self.path
            .steps()
            .first()
            .expect("a path always has a first step")
    }
}

/// What a data plane's report means for the upgrade a deployment is in the
/// middle of.
///
/// Returned rather than acted on, because emitting the next step is the
/// application's job: the domain decides whether there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeProgress {
    /// Nothing was under way, or the report says nothing about it.
    Untouched,
    /// A step landed and the next one is ready to be applied.
    ///
    /// Boxed because a deployment is large and the other three variants carry
    /// nothing: unboxed, every caller would pay that size for an answer that
    /// is usually "nothing happened".
    NextStep {
        deployment: Box<Deployment>,
        to: crate::version::Version,
    },
    /// The last step landed. The run is closed.
    Arrived,
    /// The deployment came back failed. The run is closed, and no further step
    /// is applied to an instance that is already unwell.
    GaveUp,
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

    /// Reacts to what a data plane reported about a deployment mid-upgrade.
    ///
    /// Takes no identity: the report it follows has already been checked, and
    /// this is the same act continuing rather than a new one somebody asks
    /// for. Emitting the next step is left to the caller, so that the action
    /// and the status change land in one transaction.
    fn advance_upgrade(
        &self,
        deployment_id: DeploymentId,
    ) -> impl Future<Output = Result<UpgradeProgress, CoreError>> + Send;

    /// The upgrade a deployment is in the middle of, if any.
    ///
    /// Scoped by organisation the way
    /// [`crate::deployments::ports::DeploymentService::get_deployment_for_organisation`]
    /// is: someone else's deployment is reported as not found rather than
    /// confirming it exists.
    fn upgrade_in_flight(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> impl Future<Output = Result<Option<InFlightUpgrade>, CoreError>> + Send;

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
