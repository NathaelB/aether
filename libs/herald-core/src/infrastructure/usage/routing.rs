use tracing::debug;

use crate::domain::entities::deployment::DeploymentKind;
use crate::domain::entities::usage::{CounterSample, UsageTarget};
use crate::domain::error::HeraldError;
use crate::domain::ports::UsageSource;

/// Sends each deployment to the source for the product it runs.
///
/// Keycloak has no arm here, and that is the finding rather than an omission.
/// Keycloak does expose usage -- `keycloak_user_events_total` and the Quarkus
/// HTTP counters, on the management interface -- but neither is reachable for
/// an instance this platform creates: the operator starts Keycloak with
/// `start-dev --health-enabled=true` and no `--metrics-enabled`, and the
/// `Service` it creates publishes only port 80 onto the application port,
/// while the management interface those metrics live on is port 9000. Until
/// both of those change in the operator, there is nothing to read, and a
/// Keycloak deployment reports no buckets at all rather than zeroes.
pub struct ProductUsageSource<F>
where
    F: UsageSource,
{
    ferriskey: F,
}

impl<F> ProductUsageSource<F>
where
    F: UsageSource,
{
    pub fn new(ferriskey: F) -> Self {
        Self { ferriskey }
    }
}

impl<F> UsageSource for ProductUsageSource<F>
where
    F: UsageSource,
{
    async fn sample(&self, target: &UsageTarget) -> Result<Option<CounterSample>, HeraldError> {
        match target.kind {
            DeploymentKind::Ferriskey => self.ferriskey.sample(target).await,
            DeploymentKind::Keycloak => {
                debug!(
                    deployment_id = %target.deployment_id,
                    "keycloak exposes no usage Herald can reach; reporting no buckets"
                );
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::deployment::DeploymentId;
    use crate::domain::entities::usage::UsageMetric;
    use crate::domain::ports::MockUsageSource;
    use chrono::Utc;

    fn target(kind: DeploymentKind) -> UsageTarget {
        UsageTarget {
            deployment_id: DeploymentId::new("dep-1"),
            kind,
            namespace: "aether-acme".to_string(),
        }
    }

    #[tokio::test]
    async fn a_ferriskey_deployment_is_read_by_the_ferriskey_source() {
        let mut ferriskey = MockUsageSource::new();
        ferriskey.expect_sample().times(1).returning(|_| {
            Box::pin(async {
                Ok(Some(
                    CounterSample::new(Utc::now()).with(UsageMetric::Requests, 7),
                ))
            })
        });

        let sample = ProductUsageSource::new(ferriskey)
            .sample(&target(DeploymentKind::Ferriskey))
            .await
            .expect("a reading");

        assert_eq!(
            sample.expect("counters").totals.get(&UsageMetric::Requests),
            Some(&7)
        );
    }

    /// Not an error, which would be retried and logged as a fault every cycle,
    /// and not a zero, which would be recorded as usage nobody measured.
    #[tokio::test]
    async fn a_keycloak_deployment_reports_nothing_at_all() {
        let mut ferriskey = MockUsageSource::new();
        ferriskey.expect_sample().never();

        let sample = ProductUsageSource::new(ferriskey)
            .sample(&target(DeploymentKind::Keycloak))
            .await
            .expect("no reading is not a failure");

        assert!(sample.is_none());
    }
}
