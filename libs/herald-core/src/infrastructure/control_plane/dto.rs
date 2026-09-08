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
use crate::domain::entities::deployment::{Deployment, DeploymentId};
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
}

impl From<DeploymentDto> for Deployment {
    fn from(dto: DeploymentDto) -> Self {
        Deployment {
            id: DeploymentId::new(dto.id.to_string()),
            dataplane_id: DataPlaneId::new(dto.dataplane_id.to_string()),
            name: dto.name,
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
