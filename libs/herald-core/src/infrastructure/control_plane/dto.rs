//! Wire types for the control plane HTTP API (see `GET
//! /dataplanes/{dataplane_id}/deployments`, `POST .../actions:claim` and
//! `POST .../actions:ack`). These mirror the JSON produced by
//! `aether-domain`'s `Action`/`Deployment` types without depending on that
//! crate; conversions into Herald's own domain types happen exclusively via
//! the `TryFrom`/`From` impls below, never inline in the repository.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::domain::entities::action::{Action, ActionFailureReason, ActionId};
use crate::domain::entities::dataplane::DataPlaneId;
use crate::domain::entities::deployment::{Deployment, DeploymentId, DeploymentKind};
use crate::domain::entities::logs::LogLine;
use crate::domain::entities::usage::UsagePoint;
use crate::domain::error::HeraldError;

/// Generic `{ data: T }` envelope used by every control plane response.
#[derive(Debug, Deserialize)]
pub struct DataEnvelope<T> {
    pub data: T,
}

#[derive(Debug, Deserialize)]
pub struct DeploymentDto {
    pub id: Uuid,
    pub dataplane_id: Uuid,
    pub name: String,

    /// Both defaulted rather than required. They are only needed to find the
    /// deployment's instance and read its usage, so a control plane that does
    /// not send them costs usage for that deployment -- it must not cost the
    /// listing, which is what everything else in the sync cycle runs on.
    #[serde(default)]
    pub kind: Option<DeploymentKind>,
    #[serde(default)]
    pub namespace: Option<String>,
}

impl From<DeploymentDto> for Deployment {
    fn from(dto: DeploymentDto) -> Self {
        Deployment {
            id: DeploymentId::new(dto.id.to_string()),
            dataplane_id: DataPlaneId::new(dto.dataplane_id.to_string()),
            name: dto.name,
            kind: dto.kind,
            namespace: dto.namespace,
        }
    }
}

/// One bucket on the wire, in the shape `POST
/// /deployments/{deployment_id}/usage-metrics` accepts.
#[derive(Debug, Serialize)]
pub struct UsagePointDto {
    pub metric: &'static str,
    pub bucket: DateTime<Utc>,
    pub value: u64,
}

impl From<&UsagePoint> for UsagePointDto {
    fn from(point: &UsagePoint) -> Self {
        Self {
            metric: point.metric.wire_name(),
            bucket: point.bucket.start(),
            value: point.value,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ReportUsageMetricsRequest {
    pub points: Vec<UsagePointDto>,
}

/// Request body for `POST
/// /dataplanes/{dataplane_id}/deployments/{deployment_id}/logs/{session_id}`.
///
/// Serialised straight from the domain's [`LogLine`], which already carries
/// the three fields the control plane reads, so there is no second shape of a
/// log line anywhere in this process to leave a copy in.
#[derive(Debug, Serialize)]
pub struct PushLogsRequest {
    pub lines: Vec<LogLine>,
    pub done: bool,
}

/// Sent with every heartbeat.
///
/// A body rather than a query parameter because the control plane treats a
/// heartbeat with no body as one from an older Herald that has nothing to
/// say, and leaves the last reported version standing.
#[derive(Debug, Serialize)]
pub struct HeartbeatRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator_version: Option<String>,
}

/// What the control plane says back about a batch.
///
/// `listening` is the only in-band way a reader closing the page reaches this
/// far: the batch is accepted and discarded either way, so the status code
/// says nothing. Defaults to true so an older control plane, which does not
/// send the field, keeps working as it did.
#[derive(Debug, Deserialize)]
pub struct PushLogsResponseDto {
    #[serde(default = "listening_by_default")]
    pub listening: bool,
}

fn listening_by_default() -> bool {
    true
}

impl ReportUsageMetricsRequest {
    pub fn new(points: &[UsagePoint]) -> Self {
        Self {
            points: points.iter().map(UsagePointDto::from).collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ActionPayloadDto {
    pub data: Value,
}

#[derive(Debug, Deserialize)]
pub struct ActionMetadataDto {
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ActionDto {
    pub id: Uuid,
    pub deployment_id: Uuid,
    pub dataplane_id: Uuid,
    pub action_type: String,
    pub payload: ActionPayloadDto,
    pub version: u32,
    pub metadata: ActionMetadataDto,
}

impl TryFrom<ActionDto> for Action {
    type Error = HeraldError;

    fn try_from(dto: ActionDto) -> Result<Self, Self::Error> {
        let action_type = dto.action_type.trim().to_string();
        if action_type.is_empty() {
            return Err(HeraldError::InvalidAction {
                message: format!("action {} has an empty action_type", dto.id),
            });
        }

        Ok(Action {
            id: ActionId(dto.id),
            deployment_id: DeploymentId::new(dto.deployment_id.to_string()),
            dataplane_id: DataPlaneId::new(dto.dataplane_id.to_string()),
            action_type,
            payload: dto.payload.data,
            version: dto.version,
            occurred_at: dto.metadata.created_at,
        })
    }
}

/// Request body for `POST .../actions:claim`.
#[derive(Debug, Serialize)]
pub struct ClaimActionsRequest {
    pub max: usize,
    pub lease_seconds: u64,
}

/// Mirrors the control plane's `ActionFailureReason`.
#[derive(Debug, Serialize)]
pub enum ActionFailureReasonDto {
    InvalidPayload,
    UnsupportedAction,
    PublishFailed,
    Timeout,
    InternalError(String),
}

impl From<&ActionFailureReason> for ActionFailureReasonDto {
    fn from(reason: &ActionFailureReason) -> Self {
        match reason {
            ActionFailureReason::InvalidPayload => ActionFailureReasonDto::InvalidPayload,
            ActionFailureReason::UnsupportedAction => ActionFailureReasonDto::UnsupportedAction,
            ActionFailureReason::PublishFailed => ActionFailureReasonDto::PublishFailed,
            ActionFailureReason::Timeout => ActionFailureReasonDto::Timeout,
            ActionFailureReason::InternalError(message) => {
                ActionFailureReasonDto::InternalError(message.clone())
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AckFailureDto {
    pub action_id: Uuid,
    pub reason: ActionFailureReasonDto,
}

/// Request body for `POST .../actions:ack`.
#[derive(Debug, Serialize)]
pub struct AckActionsRequest {
    pub published: Vec<Uuid>,
    pub failed: Vec<AckFailureDto>,
}

#[derive(Debug, Deserialize)]
pub struct AckActionsResponseData {
    pub acknowledged: usize,
}
