use std::future::Future;

use crate::{
    CoreError,
    deployments::DeploymentId,
    upgrades::run::{UpgradeRun, UpgradeRunId},
};

/// The record of every upgrade a deployment has ever attempted.
///
/// Separate from [`crate::deployments::ports::DeploymentRepository`]: a
/// deployment's row is the current attempt, this is every attempt, and the
/// two are written by different callers at different points in the upgrade.
pub trait UpgradeRunRepository: Send + Sync {
    /// Records a run that has just started.
    ///
    /// Always an insert, never an upsert: a run's identity is its own id, so
    /// there is nothing for a second `insert` of the same id to mean except a
    /// bug at the call site.
    fn insert(&self, run: UpgradeRun) -> impl Future<Output = Result<(), CoreError>> + Send;

    fn get(
        &self,
        id: UpgradeRunId,
    ) -> impl Future<Output = Result<Option<UpgradeRun>, CoreError>> + Send;

    /// Writes back the outcome of a run that has concluded. The identity, the
    /// deployment and both versions never move.
    fn update(&self, run: &UpgradeRun) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// A deployment's upgrade history, newest first.
    fn list_for_deployment(
        &self,
        deployment_id: DeploymentId,
    ) -> impl Future<Output = Result<Vec<UpgradeRun>, CoreError>> + Send;
}
