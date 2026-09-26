//! Service for reachability uptime queries with policy enforcement.

use aether_auth::Identity;

use crate::{
    CoreError,
    deployments::{
        DeploymentId,
        reachability_history::{DeploymentUptime, ReachabilityCheckRepository},
    },
    platform::{PlatformRight, ports::PlatformPolicy},
};

/// Service for querying deployment uptime with policy enforcement.
pub struct ReachabilityServiceImpl<R, P>
where
    R: ReachabilityCheckRepository,
    P: PlatformPolicy,
{
    repository: R,
    policy: P,
}

impl<R, P> ReachabilityServiceImpl<R, P>
where
    R: ReachabilityCheckRepository,
    P: PlatformPolicy,
{
    pub fn new(repository: R, policy: P) -> Self {
        Self { repository, policy }
    }

    /// Get uptime metrics for a deployment.
    ///
    /// Requires the caller to hold ViewEstate.
    pub async fn get_uptime(
        &self,
        identity: Identity,
        deployment_id: DeploymentId,
    ) -> Result<DeploymentUptime, CoreError> {
        // Check that the caller holds ViewEstate permission
        self.policy
            .require(identity, PlatformRight::ViewEstate)
            .await?;

        // Query the repository for uptime data
        self.repository.get_uptime(deployment_id).await
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::{
        deployments::reachability_history::{MockReachabilityCheckRepository, UptimeWindow},
        platform::fixtures::Granting,
    };

    fn a_caller() -> Identity {
        Identity::User(aether_auth::User {
            id: Uuid::nil().to_string(),
            username: "somebody".to_string(),
            email: None,
            name: None,
            roles: Vec::new(),
        })
    }

    fn a_window() -> UptimeWindow {
        UptimeWindow {
            uptime_percent: 100.0,
            covers_full_window: true,
        }
    }

    #[tokio::test]
    async fn the_service_gets_uptime_when_authorized() {
        let deployment_id = DeploymentId(Uuid::nil());
        let expected = DeploymentUptime {
            deployment_id,
            uptime_24h: a_window(),
            uptime_7d: a_window(),
            uptime_30d: a_window(),
        };

        let mut repository = MockReachabilityCheckRepository::new();
        let returned = expected.clone();
        repository.expect_get_uptime().times(1).returning(move |_| {
            Box::pin({
                let returned = returned.clone();
                async move { Ok(returned) }
            })
        });

        let service = ReachabilityServiceImpl::new(repository, Granting::everything());

        let uptime = service
            .get_uptime(a_caller(), deployment_id)
            .await
            .expect("authorized");

        assert_eq!(uptime, expected);
    }

    #[tokio::test]
    async fn the_service_refuses_without_view_estate() {
        let repository = MockReachabilityCheckRepository::new();
        let service = ReachabilityServiceImpl::new(repository, Granting::nothing());

        let result = service
            .get_uptime(a_caller(), DeploymentId(Uuid::nil()))
            .await;

        assert!(matches!(
            result,
            Err(CoreError::MissingPlatformRight { .. })
        ));
    }
}
