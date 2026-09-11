use reqwest::{Client, Response, StatusCode};

use crate::domain::{
    entities::{
        action::{AckFailure, AckOutcome, Action, ActionId},
        dataplane::DataPlaneId,
        deployment::{Deployment, DeploymentId},
        logs::{LogLine, LogStreamRequest},
        outcome::DeploymentOutcomeReport,
        usage::UsagePoint,
    },
    error::HeraldError,
    ports::{ControlPlaneRepository, LogPushOutcome},
};
use crate::infrastructure::control_plane::auth::ControlPlaneAuth;

use super::dto::{
    AckActionsRequest, AckActionsResponseData, AckFailureDto, ActionDto, ClaimActionsRequest,
    DataEnvelope, DeploymentDto, HeartbeatRequest, PushLogsRequest, PushLogsResponseDto,
    ReportUsageMetricsRequest,
};

/// Actions are claimed with a lease of this many seconds unless overridden
/// via [`HttpControlPlaneRepository::with_claim_settings`].
const DEFAULT_LEASE_SECONDS: u64 = 60;
/// At most this many actions are claimed per deployment per call unless
/// overridden via [`HttpControlPlaneRepository::with_claim_settings`].
const DEFAULT_CLAIM_MAX: usize = 50;

/// HTTP client implementation for communicating with the control plane API.
///
/// Authenticates with a bearer token whose subject must contain
/// `herald-service`, per the control plane's authorization rule.
pub struct HttpControlPlaneRepository {
    client: Client,
    base_url: String,
    auth: ControlPlaneAuth,
    claim_max: usize,
    claim_lease_seconds: u64,
    /// The version of the data plane chart this cluster is running.
    ///
    /// Reported with the heartbeat, because it is the only message that
    /// arrives whether or not there is any work. The control plane holds a
    /// release back from a cluster whose operator is too old for it, and a
    /// cluster that never says which version it runs is treated as too old
    /// for everything, which is safe and useless.
    operator_version: Option<String>,
}

impl HttpControlPlaneRepository {
    /// Creates a new instance of the HTTP control plane repository, using
    /// the default claim batch size and lease duration.
    pub fn new(base_url: impl Into<String>, auth: ControlPlaneAuth) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            auth,
            claim_max: DEFAULT_CLAIM_MAX,
            claim_lease_seconds: DEFAULT_LEASE_SECONDS,
            operator_version: None,
        }
    }

    /// Names the version this data plane runs, to be sent with every
    /// heartbeat. Anything that is not a version is dropped here rather than
    /// sent for the control plane to reject: a chart built from a branch is
    /// tagged with the branch name, and that is not a claim about a version.
    #[must_use]
    pub fn reporting_version(mut self, version: Option<String>) -> Self {
        self.operator_version = version.filter(|value| semver::Version::parse(value).is_ok());
        self
    }

    /// Overrides the `max`/`lease_seconds` sent with every `actions:claim`
    /// call.
    #[must_use]
    pub fn with_claim_settings(mut self, max: usize, lease_seconds: u64) -> Self {
        self.claim_max = max;
        self.claim_lease_seconds = lease_seconds;
        self
    }

    async fn ensure_success(response: Response, operation: &str) -> Result<Response, HeraldError> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }

        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable body>".to_string());

        Err(HeraldError::ControlPlane {
            message: format!("{operation} failed with status {status}: {body}"),
        })
    }

    fn deployments_url(&self, dataplane_id: &DataPlaneId) -> String {
        format!("{}/dataplanes/{}/deployments", self.base_url, dataplane_id)
    }

    fn claim_url(&self, dataplane_id: &DataPlaneId, deployment_id: &DeploymentId) -> String {
        format!(
            "{}/dataplanes/{}/deployments/{}/actions:claim",
            self.base_url, dataplane_id, deployment_id
        )
    }

    fn heartbeat_url(&self, dataplane_id: &DataPlaneId) -> String {
        format!("{}/dataplanes/{}/heartbeat", self.base_url, dataplane_id)
    }

    fn outcome_url(&self, dataplane_id: &DataPlaneId, deployment_id: &uuid::Uuid) -> String {
        format!(
            "{}/dataplanes/{}/deployments/{}/outcome",
            self.base_url, dataplane_id, deployment_id
        )
    }

    /// Not nested under the data plane, unlike every other call here: the
    /// control plane scopes this one to the deployment being reported on.
    fn usage_url(&self, deployment_id: &DeploymentId) -> String {
        format!(
            "{}/deployments/{}/usage-metrics",
            self.base_url, deployment_id
        )
    }

    fn logs_url(&self, request: &LogStreamRequest) -> String {
        format!(
            "{}/dataplanes/{}/deployments/{}/logs/{}",
            self.base_url, request.dataplane_id, request.deployment_id, request.session_id
        )
    }

    fn ack_url(&self, dataplane_id: &DataPlaneId, deployment_id: &DeploymentId) -> String {
        format!(
            "{}/dataplanes/{}/deployments/{}/actions:ack",
            self.base_url, dataplane_id, deployment_id
        )
    }
}

impl ControlPlaneRepository for HttpControlPlaneRepository {
    async fn list_deployments(
        &self,
        dataplane_id: &DataPlaneId,
    ) -> Result<Vec<Deployment>, HeraldError> {
        let response = self
            .client
            .get(self.deployments_url(dataplane_id))
            .bearer_auth(self.auth.bearer().await?)
            .send()
            .await
            .map_err(|err| HeraldError::ControlPlane {
                message: format!("failed to list deployments: {err}"),
            })?;

        let response = Self::ensure_success(response, "list deployments").await?;

        let envelope: DataEnvelope<Vec<DeploymentDto>> =
            response
                .json()
                .await
                .map_err(|err| HeraldError::ControlPlane {
                    message: format!("failed to parse deployments response: {err}"),
                })?;

        Ok(envelope.data.into_iter().map(Deployment::from).collect())
    }

    async fn claim_actions(
        &self,
        dataplane_id: &DataPlaneId,
        deployment_id: &DeploymentId,
    ) -> Result<Vec<Action>, HeraldError> {
        let body = ClaimActionsRequest {
            max: self.claim_max,
            lease_seconds: self.claim_lease_seconds,
        };

        let response = self
            .client
            .post(self.claim_url(dataplane_id, deployment_id))
            .bearer_auth(self.auth.bearer().await?)
            .json(&body)
            .send()
            .await
            .map_err(|err| HeraldError::ControlPlane {
                message: format!("failed to claim actions: {err}"),
            })?;

        let response = Self::ensure_success(response, "claim actions").await?;

        let envelope: DataEnvelope<Vec<ActionDto>> =
            response
                .json()
                .await
                .map_err(|err| HeraldError::ControlPlane {
                    message: format!("failed to parse claim response: {err}"),
                })?;

        envelope
            .data
            .into_iter()
            .map(Action::try_from)
            .collect::<Result<Vec<_>, _>>()
    }

    async fn ack_actions(
        &self,
        dataplane_id: &DataPlaneId,
        deployment_id: &DeploymentId,
        published: Vec<ActionId>,
        failed: Vec<AckFailure>,
    ) -> Result<AckOutcome, HeraldError> {
        let body = AckActionsRequest {
            published: published.into_iter().map(|id| id.0).collect(),
            failed: failed
                .iter()
                .map(|failure| AckFailureDto {
                    action_id: failure.action_id.0,
                    reason: (&failure.reason).into(),
                })
                .collect(),
        };

        let response = self
            .client
            .post(self.ack_url(dataplane_id, deployment_id))
            .bearer_auth(self.auth.bearer().await?)
            .json(&body)
            .send()
            .await
            .map_err(|err| HeraldError::ControlPlane {
                message: format!("failed to ack actions: {err}"),
            })?;

        let response = Self::ensure_success(response, "ack actions").await?;

        let envelope: DataEnvelope<AckActionsResponseData> =
            response
                .json()
                .await
                .map_err(|err| HeraldError::ControlPlane {
                    message: format!("failed to parse ack response: {err}"),
                })?;

        Ok(AckOutcome {
            acknowledged: envelope.data.acknowledged,
        })
    }

    async fn send_heartbeat(&self, dp_id: &DataPlaneId) -> Result<(), HeraldError> {
        let response = self
            .client
            .post(self.heartbeat_url(dp_id))
            .bearer_auth(self.auth.bearer().await?)
            .json(&HeartbeatRequest {
                operator_version: self.operator_version.clone(),
            })
            .send()
            .await
            .map_err(|e| HeraldError::ControlPlane {
                message: format!("send_heartbeat request failed: {e}"),
            })?;

        Self::ensure_success(response, "send_heartbeat").await?;

        Ok(())
    }

    async fn report_outcome(
        &self,
        dp_id: &DataPlaneId,
        report: &DeploymentOutcomeReport,
    ) -> Result<(), HeraldError> {
        let response = self
            .client
            .post(self.outcome_url(dp_id, &report.deployment_id))
            .bearer_auth(self.auth.bearer().await?)
            .json(&serde_json::json!({
                "outcome": report.outcome,
                "version": report.version,
            }))
            .send()
            .await
            .map_err(|e| HeraldError::ControlPlane {
                message: format!("report_outcome request failed: {e}"),
            })?;

        Self::ensure_success(response, "report_outcome").await?;

        Ok(())
    }

    async fn report_usage(
        &self,
        deployment_id: &DeploymentId,
        points: &[UsagePoint],
    ) -> Result<(), HeraldError> {
        let response = self
            .client
            .post(self.usage_url(deployment_id))
            .bearer_auth(self.auth.bearer().await?)
            .json(&ReportUsageMetricsRequest::new(points))
            .send()
            .await
            .map_err(|e| HeraldError::ControlPlane {
                message: format!("report_usage request failed: {e}"),
            })?;

        Self::ensure_success(response, "report_usage").await?;

        Ok(())
    }

    async fn push_log_lines(
        &self,
        request: &LogStreamRequest,
        lines: Vec<LogLine>,
        done: bool,
    ) -> Result<LogPushOutcome, HeraldError> {
        let response = self
            .client
            .post(self.logs_url(request))
            .bearer_auth(self.auth.bearer().await?)
            .json(&PushLogsRequest { lines, done })
            .send()
            .await
            .map_err(|e| HeraldError::ControlPlane {
                message: format!("push_log_lines request failed: {e}"),
            })?;

        let status = response.status();

        // A session that never existed, or one the control plane has already
        // forgotten. Kept alongside the body check below because they are
        // different failures and only one of them has a body to read.
        if status == StatusCode::NOT_FOUND || status == StatusCode::GONE {
            return Ok(LogPushOutcome::SessionGone);
        }

        let response = Self::ensure_success(response, "push_log_lines").await?;

        // The control plane accepts a batch for a session nobody is reading
        // and discards it, so the status says nothing about whether anyone is
        // still there. The body does, and it is what lets this stop within a
        // batch instead of running to the session ceiling.
        let answer: DataEnvelope<PushLogsResponseDto> =
            response
                .json()
                .await
                .map_err(|e| HeraldError::ControlPlane {
                    message: format!("push_log_lines answered with something unreadable: {e}"),
                })?;

        Ok(if answer.data.listening {
            LogPushOutcome::Relayed
        } else {
            LogPushOutcome::SessionGone
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::Method::{GET, POST};
    use httpmock::MockServer;
    use serde_json::json;

    fn repo(server: &MockServer) -> HttpControlPlaneRepository {
        HttpControlPlaneRepository::new(
            server.base_url(),
            ControlPlaneAuth::Static("herald-service-token".to_string()),
        )
    }

    #[tokio::test]
    async fn list_deployments_maps_response_into_domain_deployments() {
        let server = MockServer::start();
        let dataplane_id = DataPlaneId::new("11111111-1111-1111-1111-111111111111");

        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/dataplanes/11111111-1111-1111-1111-111111111111/deployments")
                .header("authorization", "Bearer herald-service-token");
            then.status(200).json_body(json!({
                "data": [
                    {
                        "id": "22222222-2222-2222-2222-222222222222",
                        "dataplane_id": "11111111-1111-1111-1111-111111111111",
                        "name": "my-deployment"
                    }
                ]
            }));
        });

        let deployments = repo(&server)
            .list_deployments(&dataplane_id)
            .await
            .expect("list_deployments succeeds");

        mock.assert();
        assert_eq!(deployments.len(), 1);
        assert_eq!(
            deployments[0].id,
            DeploymentId::new("22222222-2222-2222-2222-222222222222")
        );
        assert_eq!(deployments[0].name, "my-deployment");
    }

    #[tokio::test]
    async fn claim_actions_sends_max_and_lease_and_maps_actions() {
        let server = MockServer::start();
        let dataplane_id = DataPlaneId::new("dp-1");
        let deployment_id = DeploymentId::new("dep-1");

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/dataplanes/dp-1/deployments/dep-1/actions:claim")
                .header("authorization", "Bearer herald-service-token")
                .json_body(json!({"max": 50, "lease_seconds": 60}));
            then.status(200).json_body(json!({
                "data": [
                    {
                        "id": "33333333-3333-3333-3333-333333333333",
                        "deployment_id": "44444444-4444-4444-4444-444444444444",
                        "dataplane_id": "11111111-1111-1111-1111-111111111111",
                        "action_type": "deployment.create",
                        "target": {"kind": "Deployment", "id": "44444444-4444-4444-4444-444444444444"},
                        "payload": {"data": {"replicas": 3}},
                        "version": 2,
                        "status": "Pending",
                        "metadata": {
                            "source": "System",
                            "created_at": "2026-01-01T00:00:00Z",
                            "constraints": {}
                        },
                        "leased_until": "2026-01-01T00:01:00Z"
                    }
                ]
            }));
        });

        let actions = repo(&server)
            .claim_actions(&dataplane_id, &deployment_id)
            .await
            .expect("claim_actions succeeds");

        mock.assert();
        assert_eq!(actions.len(), 1);
        let action = &actions[0];
        assert_eq!(
            action.id,
            ActionId(uuid::Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap())
        );
        assert_eq!(
            action.deployment_id,
            DeploymentId::new("44444444-4444-4444-4444-444444444444")
        );
        assert_eq!(
            action.dataplane_id,
            DataPlaneId::new("11111111-1111-1111-1111-111111111111")
        );
        assert_eq!(action.action_type, "deployment.create");
        assert_eq!(action.version, 2);
        assert_eq!(action.payload, json!({"replicas": 3}));
    }

    #[tokio::test]
    async fn ack_actions_sends_published_and_failed_and_returns_acknowledged_count() {
        let server = MockServer::start();
        let dataplane_id = DataPlaneId::new("dp-1");
        let deployment_id = DeploymentId::new("dep-1");

        let published_id =
            ActionId(uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap());
        let failed_id =
            ActionId(uuid::Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap());

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/dataplanes/dp-1/deployments/dep-1/actions:ack")
                .header("authorization", "Bearer herald-service-token")
                .json_body(json!({
                    "published": ["11111111-1111-1111-1111-111111111111"],
                    "failed": [
                        {
                            "action_id": "22222222-2222-2222-2222-222222222222",
                            "reason": "PublishFailed"
                        }
                    ]
                }));
            then.status(200)
                .json_body(json!({"data": {"acknowledged": 2}}));
        });

        let outcome = repo(&server)
            .ack_actions(
                &dataplane_id,
                &deployment_id,
                vec![published_id],
                vec![AckFailure {
                    action_id: failed_id,
                    reason: crate::domain::entities::action::ActionFailureReason::PublishFailed,
                }],
            )
            .await
            .expect("ack_actions succeeds");

        mock.assert();
        assert_eq!(outcome.acknowledged, 2);
    }

    /// Herald cannot find a deployment's instance without both of these, and
    /// they already travel on this response -- they were simply not being
    /// read.
    #[tokio::test]
    async fn list_deployments_reads_the_product_and_the_namespace() {
        let server = MockServer::start();
        let dataplane_id = DataPlaneId::new("dp-1");

        server.mock(|when, then| {
            when.method(GET).path("/dataplanes/dp-1/deployments");
            then.status(200).json_body(json!({
                "data": [
                    {
                        "id": "22222222-2222-2222-2222-222222222222",
                        "dataplane_id": "11111111-1111-1111-1111-111111111111",
                        "name": "acme-prod",
                        "kind": "ferriskey",
                        "namespace": "aether-acme-prod"
                    }
                ]
            }));
        });

        let deployments = repo(&server)
            .list_deployments(&dataplane_id)
            .await
            .expect("list_deployments succeeds");

        let target = deployments[0].usage_target().expect("a usage target");
        assert_eq!(
            target.kind,
            crate::domain::entities::deployment::DeploymentKind::Ferriskey
        );
        assert_eq!(target.namespace, "aether-acme-prod");
    }

    #[tokio::test]
    async fn report_usage_posts_buckets_against_the_deployment() {
        use crate::domain::entities::usage::{UsageBucket, UsageMetric, UsagePoint};
        use chrono::{DateTime, Utc};

        let server = MockServer::start();
        let deployment_id = DeploymentId::new("dep-1");
        let bucket = UsageBucket::containing(
            DateTime::parse_from_rfc3339("2026-01-01T10:05:42Z")
                .unwrap()
                .with_timezone(&Utc),
        );

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/deployments/dep-1/usage-metrics")
                .header("authorization", "Bearer herald-service-token")
                .json_body(json!({
                    "points": [
                        {"metric": "requests", "bucket": "2026-01-01T10:05:00Z", "value": 42},
                        {"metric": "token_events", "bucket": "2026-01-01T10:05:00Z", "value": 0}
                    ]
                }));
            then.status(200).json_body(json!({"data": {"recorded": 2}}));
        });

        repo(&server)
            .report_usage(
                &deployment_id,
                &[
                    UsagePoint {
                        metric: UsageMetric::Requests,
                        bucket,
                        value: 42,
                    },
                    UsagePoint {
                        metric: UsageMetric::TokenEvents,
                        bucket,
                        value: 0,
                    },
                ],
            )
            .await
            .expect("report_usage succeeds");

        mock.assert();
    }

    fn log_request() -> crate::domain::entities::logs::LogStreamRequest {
        use crate::domain::entities::deployment::DeploymentKind;
        use crate::domain::entities::logs::LogSessionId;

        crate::domain::entities::logs::LogStreamRequest {
            deployment_id: DeploymentId::new("dep-1"),
            dataplane_id: DataPlaneId::new("dp-1"),
            namespace: "aether-acme".to_string(),
            kind: DeploymentKind::Ferriskey,
            session_id: LogSessionId(
                uuid::Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
            ),
            since_minutes: 10,
        }
    }

    #[tokio::test]
    async fn push_log_lines_posts_a_batch_against_the_session() {
        use chrono::{DateTime, Utc};

        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path(
                    "/dataplanes/dp-1/deployments/dep-1/logs/33333333-3333-3333-3333-333333333333",
                )
                .header("authorization", "Bearer herald-service-token")
                .json_body(json!({
                    "lines": [
                        {
                            "at": "2026-09-11T10:00:00Z",
                            "source": "ferriskey-api",
                            "message": "started"
                        }
                    ],
                    "done": false
                }));
            then.status(200)
                .json_body(json!({"data": {"relayed": 1, "listening": true}}));
        });

        let outcome = repo(&server)
            .push_log_lines(
                &log_request(),
                vec![LogLine {
                    at: DateTime::parse_from_rfc3339("2026-09-11T10:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                    source: "ferriskey-api".to_string(),
                    message: "started".to_string(),
                }],
                false,
            )
            .await
            .expect("push_log_lines succeeds");

        mock.assert();
        assert_eq!(outcome, LogPushOutcome::Relayed);
    }

    /// A cluster that never says which version it runs is held back from
    /// every release that names a minimum, so this has to travel.
    #[tokio::test]
    async fn the_heartbeat_carries_the_version_this_cluster_runs() {
        let server = MockServer::start();
        let dataplane_id = DataPlaneId::new("dp-1");

        let heartbeat = server.mock(|when, then| {
            when.method(POST)
                .path("/dataplanes/dp-1/heartbeat")
                .json_body(json!({"operator_version": "1.4.0"}));
            then.status(200)
                .json_body(json!({"data": {"recorded": true}}));
        });

        repo(&server)
            .reporting_version(Some("1.4.0".to_string()))
            .send_heartbeat(&dataplane_id)
            .await
            .expect("reported");

        heartbeat.assert();
    }

    /// A chart built from a branch is tagged with the branch name. Sending
    /// that would have the control plane refuse a heartbeat over something
    /// that is not a claim about a version at all.
    #[tokio::test]
    async fn a_tag_that_is_not_a_version_is_not_reported_as_one() {
        let server = MockServer::start();
        let dataplane_id = DataPlaneId::new("dp-1");

        let heartbeat = server.mock(|when, then| {
            when.method(POST)
                .path("/dataplanes/dp-1/heartbeat")
                .json_body(json!({}));
            then.status(200)
                .json_body(json!({"data": {"recorded": true}}));
        });

        repo(&server)
            .reporting_version(Some("main".to_string()))
            .send_heartbeat(&dataplane_id)
            .await
            .expect("reported");

        heartbeat.assert();
    }

    /// The only in-band way a reader closing the page can reach this far. It
    /// must not read as a failure, because a failure is retried.
    #[tokio::test]
    async fn a_session_the_control_plane_no_longer_knows_is_not_an_error() {
        let server = MockServer::start();

        server.mock(|when, then| {
            when.method(POST);
            then.status(404).body("no such session");
        });

        let outcome = repo(&server)
            .push_log_lines(&log_request(), Vec::new(), true)
            .await
            .expect("a closed session is not a failure");

        assert_eq!(outcome, LogPushOutcome::SessionGone);
    }

    /// The batch was accepted, and nobody was there to receive it. The status
    /// cannot say so, because accepting and discarding is the correct thing
    /// for the control plane to do; the body is the only place it fits.
    #[tokio::test]
    async fn a_batch_nobody_received_reads_as_a_closed_session() {
        let server = MockServer::start();

        server.mock(|when, then| {
            when.method(POST);
            then.status(200)
                .json_body(json!({"data": {"relayed": 3, "listening": false}}));
        });

        let outcome = repo(&server)
            .push_log_lines(&log_request(), Vec::new(), false)
            .await
            .expect("accepted");

        assert_eq!(outcome, LogPushOutcome::SessionGone);
    }

    /// An older control plane does not send the field. Reading its silence as
    /// "nobody is reading" would stop every session after one batch.
    #[tokio::test]
    async fn a_control_plane_that_says_nothing_is_taken_as_still_listening() {
        let server = MockServer::start();

        server.mock(|when, then| {
            when.method(POST);
            then.status(200).json_body(json!({"data": {"relayed": 1}}));
        });

        let outcome = repo(&server)
            .push_log_lines(&log_request(), Vec::new(), false)
            .await
            .expect("accepted");

        assert_eq!(outcome, LogPushOutcome::Relayed);
    }

    #[tokio::test]
    async fn non_2xx_response_maps_to_control_plane_error() {
        let server = MockServer::start();
        let dataplane_id = DataPlaneId::new("dp-1");

        server.mock(|when, then| {
            when.method(GET).path("/dataplanes/dp-1/deployments");
            then.status(401).body("missing herald-service credentials");
        });

        let result = repo(&server).list_deployments(&dataplane_id).await;

        match result {
            Err(HeraldError::ControlPlane { message }) => {
                assert!(message.contains("401"));
                assert!(message.contains("missing herald-service credentials"));
            }
            other => panic!("expected ControlPlane error, got {other:?}"),
        }
    }
}
