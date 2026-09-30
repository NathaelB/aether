use aether_auth::Identity;
use chrono::{DateTime, Duration, Utc};

use crate::{
    CoreError,
    deployments::{
        DeploymentId,
        reachability_history::{
            DeploymentDowntime, DeploymentUptime, ReachabilityCheckRepository, availability,
            downtime_intervals,
        },
    },
    platform::{PlatformRight, ports::PlatformPolicy},
};

const THIRTY_DAYS: Duration = Duration::days(30);

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

    pub async fn get_uptime(
        &self,
        identity: Identity,
        deployment_id: DeploymentId,
    ) -> Result<DeploymentUptime, CoreError> {
        self.policy
            .require(identity, PlatformRight::ViewEstate)
            .await?;

        let now = Utc::now();
        let checks = self
            .repository
            .get_checks_since(deployment_id, now - THIRTY_DAYS)
            .await?;

        Ok(DeploymentUptime {
            deployment_id,
            uptime_24h: availability(&checks, now, Duration::hours(24)),
            uptime_7d: availability(&checks, now, Duration::days(7)),
            uptime_30d: availability(&checks, now, THIRTY_DAYS),
        })
    }

    pub async fn get_downtime(
        &self,
        identity: Identity,
        deployment_id: DeploymentId,
        since: DateTime<Utc>,
    ) -> Result<DeploymentDowntime, CoreError> {
        self.policy
            .require(identity, PlatformRight::ViewEstate)
            .await?;

        let checks = self
            .repository
            .get_checks_since(deployment_id, since)
            .await?;

        Ok(DeploymentDowntime {
            deployment_id,
            intervals: downtime_intervals(&checks),
        })
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::{
        deployments::reachability_history::MockReachabilityCheckRepository,
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

    #[tokio::test]
    async fn the_service_derives_uptime_from_one_read_when_authorized() {
        use crate::deployments::reachability_history::ReachabilityCheck;

        let deployment_id = DeploymentId(Uuid::nil());
        let now = Utc::now();
        let checks: Vec<ReachabilityCheck> = (0..=120)
            .map(|minute| ReachabilityCheck {
                deployment_id,
                checked_at: now - Duration::minutes(120 - minute),
                reachable: minute != 60,
            })
            .collect();

        let mut repository = MockReachabilityCheckRepository::new();
        repository
            .expect_get_checks_since()
            .times(1)
            .withf(move |id, since| {
                *id == deployment_id && (now - *since - THIRTY_DAYS).abs() < Duration::seconds(5)
            })
            .returning(move |_, _| {
                let checks = checks.clone();
                Box::pin(async move { Ok(checks) })
            });

        let service = ReachabilityServiceImpl::new(repository, Granting::everything());

        let uptime = service
            .get_uptime(a_caller(), deployment_id)
            .await
            .expect("authorized");

        assert_eq!(uptime.deployment_id, deployment_id);
        let percent = uptime.uptime_24h.uptime_percent.expect("observed");
        assert!((percent - 100.0 * (1.0 - 1.0 / 120.0)).abs() < 0.01);
        assert_eq!(
            uptime.uptime_7d.uptime_percent.map(|p| p.round()),
            Some(99.0)
        );
        assert!(!uptime.uptime_30d.covers_full_window);
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

    #[tokio::test]
    async fn the_service_derives_downtime_when_authorized() {
        use crate::deployments::reachability_history::ReachabilityCheck;

        let deployment_id = DeploymentId(Uuid::nil());
        let since = Utc::now();
        let failed = ReachabilityCheck {
            deployment_id,
            checked_at: since,
            reachable: false,
        };

        let mut repository = MockReachabilityCheckRepository::new();
        repository
            .expect_get_checks_since()
            .times(1)
            .returning(move |_, _| {
                let failed = failed.clone();
                Box::pin(async move { Ok(vec![failed]) })
            });

        let service = ReachabilityServiceImpl::new(repository, Granting::everything());

        let downtime = service
            .get_downtime(a_caller(), deployment_id, since)
            .await
            .expect("authorized");

        assert_eq!(downtime.deployment_id, deployment_id);
        assert_eq!(downtime.intervals.len(), 1);
        assert_eq!(downtime.intervals[0].ended_at, None);
    }

    #[tokio::test]
    async fn downtime_is_refused_without_view_estate() {
        let repository = MockReachabilityCheckRepository::new();
        let service = ReachabilityServiceImpl::new(repository, Granting::nothing());

        let result = service
            .get_downtime(a_caller(), DeploymentId(Uuid::nil()), Utc::now())
            .await;

        assert!(matches!(
            result,
            Err(CoreError::MissingPlatformRight { .. })
        ));
    }
}
