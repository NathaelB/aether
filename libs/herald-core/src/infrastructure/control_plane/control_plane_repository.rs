use reqwest::{Client, Response};

use crate::domain::{
    entities::{
        action::{AckFailure, AckOutcome, Action, ActionId},
        dataplane::DataPlaneId,
        deployment::{Deployment, DeploymentId},
    },
    error::HeraldError,
    ports::ControlPlaneRepository,
};
use crate::infrastructure::control_plane::auth::ControlPlaneAuth;

use super::dto::{
    AckActionsRequest, AckActionsResponseData, AckFailureDto, ActionDto, ClaimActionsRequest,
    DataEnvelope, DeploymentDto,
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
        }
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
            .send()
            .await
            .map_err(|e| HeraldError::ControlPlane {
                message: format!("send_heartbeat request failed: {e}"),
            })?;

        Self::ensure_success(response, "send_heartbeat").await?;

        Ok(())
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
