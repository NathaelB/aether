//! Deployment reachability operations for probes and the API.
//!
//! Recording check results is called by the background probe with no Identity.
//! Reading uptime metrics is a platform endpoint: requires identity-gating and ViewEstate.

use aether_auth::Identity;
use aether_domain::deployments::{
    DeploymentId,
    reachability_history::{
        DeploymentUptime, ReachabilityCheck, ReachabilityCheckRepository,
        service::ReachabilityServiceImpl,
    },
};
use aether_macros::transactional;
use chrono::Duration;

use crate::{AetherService, CoreError, policy::PlatformRightsPolicy};

impl AetherService {
    /// Record a deployment reachability check result.
    ///
    /// Called by the background reachability probe (no Identity). Stores the raw
    /// pass/fail result for later uptime computation.
    #[transactional(reachability_check)]
    pub async fn record_reachability_check(
        &self,
        check: ReachabilityCheck,
    ) -> Result<(), CoreError> {
        reachability_check_repository.record_check(check).await
    }

    /// Get uptime metrics for a deployment.
    ///
    /// Requires the caller to hold ViewEstate. Computes uptime percentages over
    /// 24h, 7d, and 30d windows from recorded check history.
    #[transactional(reachability_check)]
    pub async fn get_deployment_uptime(
        &self,
        identity: Identity,
        deployment_id: DeploymentId,
    ) -> Result<DeploymentUptime, CoreError> {
        let service = ReachabilityServiceImpl::new(
            reachability_check_repository,
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
        );

        service.get_uptime(identity, deployment_id).await
    }

    /// Purge reachability check records older than the retention window.
    /// Returns the number of rows deleted.
    #[transactional(reachability_check)]
    pub async fn purge_old_reachability_checks(
        &self,
        retention: Duration,
    ) -> Result<u64, CoreError> {
        reachability_check_repository
            .purge_old_checks(retention)
            .await
    }
}
