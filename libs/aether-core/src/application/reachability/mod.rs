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
    #[transactional(reachability_check)]
    pub async fn record_reachability_check(
        &self,
        check: ReachabilityCheck,
    ) -> Result<(), CoreError> {
        reachability_check_repository.record_check(check).await
    }

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
