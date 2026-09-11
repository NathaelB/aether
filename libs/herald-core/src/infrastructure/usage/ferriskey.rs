use std::time::Duration;

use chrono::Utc;
use reqwest::Client;

use crate::domain::entities::usage::{CounterSample, UsageMetric, UsageTarget};
use crate::domain::error::HeraldError;
use crate::domain::ports::UsageSource;
use crate::infrastructure::usage::prometheus::{label, sum_where};

/// Where a FerrisKey instance's metrics live, with `{namespace}` and
/// `{service}` filled in per deployment.
///
/// The default follows what the operator actually creates: an
/// `IdentityInstance` named `deployment-<deployment id>`, an API `Service`
/// named `<instance>-api` publishing port 3333, and a server started with
/// `SERVER_ROOT_PATH=/api`. Reaching it over cluster DNS means Herald needs no
/// Kubernetes credentials to collect usage -- and it is a template rather than
/// a constant because that convention belongs to another component, and an
/// operator whose convention has moved needs a way to say so that is not a
/// rebuild.
pub const DEFAULT_METRICS_URL: &str =
    "http://{service}.{namespace}.svc.cluster.local:3333/api/metrics";

/// The counter `axum-prometheus` registers for every handled request. It is
/// the only counter FerrisKey exposes: the project registers no metrics of its
/// own, so anything Herald reports about a FerrisKey instance has to be
/// derived from this one family.
const REQUESTS: &str = "axum_http_requests_total";

/// The route template the OIDC token endpoint is registered under. Matched on
/// the suffix so that the realm placeholder and the configured root path do
/// not have to be guessed -- and so that `.../token/introspect`, which is a
/// different endpoint, is not counted as a token being issued.
const TOKEN_ENDPOINT: &str = "/protocol/openid-connect/token";

/// A read that has not answered within this is treated as unreachable.
///
/// Short, because the alternative to giving up is holding the cycle: every
/// other deployment's reading is queued behind this one, and a bucket arrives
/// late for all of them rather than missing for one.
const TIMEOUT: Duration = Duration::from_secs(5);

pub struct FerriskeyUsageSource {
    client: Client,
    metrics_url: String,
}

impl FerriskeyUsageSource {
    pub fn new(metrics_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            metrics_url: metrics_url.into(),
        }
    }

    fn url_for(&self, target: &UsageTarget) -> String {
        self.metrics_url
            .replace(
                "{service}",
                &format!("deployment-{}-api", target.deployment_id),
            )
            .replace("{namespace}", &target.namespace)
    }
}

impl UsageSource for FerriskeyUsageSource {
    async fn sample(&self, target: &UsageTarget) -> Result<Option<CounterSample>, HeraldError> {
        let url = self.url_for(target);

        let response = self
            .client
            .get(&url)
            .timeout(TIMEOUT)
            .send()
            .await
            .map_err(|err| HeraldError::UsageSource {
                message: format!("could not read {url}: {err}"),
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(HeraldError::UsageSource {
                message: format!("{url} returned {status}"),
            });
        }

        let exposition = response
            .text()
            .await
            .map_err(|err| HeraldError::UsageSource {
                message: format!("could not read the body of {url}: {err}"),
            })?;

        let Some(requests) = sum_where(&exposition, REQUESTS, |_| true) else {
            // The instance answered, but with no counter Herald knows how to
            // read. That is a fact about the build, not about this minute.
            return Ok(None);
        };

        let mut sample = CounterSample::new(Utc::now()).with(UsageMetric::Requests, requests);

        // Only when the series is actually there. A token endpoint whose route
        // template has moved and a token endpoint nobody has called look
        // identical from here, and only one of them is a zero.
        if let Some(token_events) = sum_where(&exposition, REQUESTS, |labels| {
            label(labels, "endpoint").is_some_and(|endpoint| endpoint.ends_with(TOKEN_ENDPOINT))
        }) {
            sample = sample.with(UsageMetric::TokenEvents, token_events);
        }

        Ok(Some(sample))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::deployment::{DeploymentId, DeploymentKind};
    use httpmock::prelude::*;

    const EXPOSITION: &str = r#"
# TYPE axum_http_requests_total counter
axum_http_requests_total{endpoint="/api/realms/{realm_name}/protocol/openid-connect/token",method="POST",status="200"} 12
axum_http_requests_total{endpoint="/api/realms/{realm_name}/protocol/openid-connect/token/introspect",method="POST",status="200"} 3
axum_http_requests_total{endpoint="/api/health",method="GET",status="200"} 400
"#;

    fn target() -> UsageTarget {
        UsageTarget {
            deployment_id: DeploymentId::new("22222222-2222-2222-2222-222222222222"),
            kind: DeploymentKind::Ferriskey,
            namespace: "aether-acme".to_string(),
        }
    }

    fn source(server: &MockServer) -> FerriskeyUsageSource {
        FerriskeyUsageSource::new(format!("{}/api/metrics", server.base_url()))
    }

    #[tokio::test]
    async fn a_reading_carries_the_request_and_token_counters() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/api/metrics");
                then.status(200).body(EXPOSITION);
            })
            .await;

        let sample = source(&server)
            .sample(&target())
            .await
            .expect("a reading")
            .expect("counters");

        assert_eq!(sample.totals.get(&UsageMetric::Requests), Some(&415));
        assert_eq!(sample.totals.get(&UsageMetric::TokenEvents), Some(&12));
    }

    /// FerrisKey registers no login or active-user counter, so a reading must
    /// not carry either. An empty entry would be reported as a zero and read
    /// as "nobody logged in".
    #[tokio::test]
    async fn a_reading_carries_nothing_ferriskey_does_not_expose() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/api/metrics");
                then.status(200).body(EXPOSITION);
            })
            .await;

        let sample = source(&server)
            .sample(&target())
            .await
            .expect("a reading")
            .expect("counters");

        assert_eq!(sample.totals.get(&UsageMetric::Logins), None);
        assert_eq!(sample.totals.get(&UsageMetric::ActiveUsers), None);
    }

    #[tokio::test]
    async fn an_instance_with_no_token_traffic_yet_reports_no_token_bucket() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/api/metrics");
                then.status(200).body(
                    r#"axum_http_requests_total{endpoint="/api/health",method="GET",status="200"} 4"#,
                );
            })
            .await;

        let sample = source(&server)
            .sample(&target())
            .await
            .expect("a reading")
            .expect("counters");

        assert_eq!(sample.totals.get(&UsageMetric::Requests), Some(&4));
        assert_eq!(sample.totals.get(&UsageMetric::TokenEvents), None);
    }

    /// An instance that cannot be reached is an error, never an empty reading:
    /// an empty reading is folded into a bucket, and an error is not.
    #[tokio::test]
    async fn an_instance_that_cannot_be_reached_is_an_error() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/api/metrics");
                then.status(503).body("no healthy upstream");
            })
            .await;

        let error = source(&server)
            .sample(&target())
            .await
            .expect_err("an unreachable instance must not produce a reading");

        assert!(
            matches!(error, HeraldError::UsageSource { .. }),
            "{error:?}"
        );
    }

    /// An instance that answers with no counter Herald can read is a standing
    /// fact about the build, not a failure to be retried.
    #[tokio::test]
    async fn an_instance_exposing_no_known_counter_reports_nothing() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/api/metrics");
                then.status(200)
                    .body("# TYPE something_else counter\nsomething_else 1\n");
            })
            .await;

        let sample = source(&server).sample(&target()).await.expect("a reading");

        assert!(sample.is_none());
    }

    #[test]
    fn the_default_url_follows_the_service_the_operator_creates() {
        let source = FerriskeyUsageSource::new(DEFAULT_METRICS_URL);

        assert_eq!(
            source.url_for(&target()),
            "http://deployment-22222222-2222-2222-2222-222222222222-api.aether-acme.svc.cluster.local:3333/api/metrics"
        );
    }
}
