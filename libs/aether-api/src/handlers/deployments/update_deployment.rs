use aether_core::deployments::{
    Deployment, DeploymentKind, DeploymentName, DeploymentStatus, DeploymentVersion,
    commands::UpdateDeploymentCommand, ports::DeploymentService,
};
use axum::{Json, extract::State};
use axum_extra::routing::TypedPath;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct UpdateDeploymentRequest {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub version: Option<String>,
    pub status: Option<String>,
    pub namespace: Option<String>,
    pub deployed_at: Option<String>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct UpdateDeploymentResponse {
    data: Deployment,
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments/{deployment_id}")]
pub struct UpdateDeploymentRoute {
    pub organisation_id: Uuid,
    pub deployment_id: Uuid,
}

#[utoipa::path(
    patch,
    path = "/{organisation_id}/deployments/{deployment_id}",
    summary = "update deployment",
    tag = "deployments",
    description = "Update a deployment within the specified organisation.",
    request_body = UpdateDeploymentRequest,
    params(UpdateDeploymentRoute),
    responses(
        (status = 200, description = "Deployment updated successfully", body = UpdateDeploymentResponse),
        (status = 400, description = "Invalid request data", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_deployment_handler(
    UpdateDeploymentRoute {
        organisation_id,
        deployment_id,
    }: UpdateDeploymentRoute,
    State(state): State<AppState>,
    Json(request): Json<UpdateDeploymentRequest>,
) -> Result<Response<UpdateDeploymentResponse>, ApiError> {
    let organisation_id = organisation_id.into();
    let deployment_id = deployment_id.into();

    let mut command = UpdateDeploymentCommand::new();
    if let Some(name) = request.name {
        command = command.with_name(DeploymentName(name));
    }
    if let Some(kind) = request.kind {
        let kind = DeploymentKind::try_from(kind.as_str()).map_err(|e| ApiError::BadRequest {
            reason: e.to_string(),
        })?;
        command = command.with_kind(kind);
    }
    if let Some(version) = request.version {
        command = command.with_version(DeploymentVersion(version));
    }
    if let Some(status) = request.status {
        let status =
            DeploymentStatus::try_from(status.as_str()).map_err(|e| ApiError::BadRequest {
                reason: e.to_string(),
            })?;
        command = command.with_status(status);
    }
    if let Some(namespace) = request.namespace {
        command = command.with_namespace(namespace);
    }
    if let Some(deployed_at) = request.deployed_at {
        let parsed = DateTime::parse_from_rfc3339(&deployed_at)
            .map_err(|e| ApiError::BadRequest {
                reason: format!("Invalid deployed_at: {}", e),
            })?
            .with_timezone(&Utc);
        command = command.with_deployed_at(Some(parsed));
    }

    let deployment = state
        .service
        .update_deployment_for_organisation(organisation_id, deployment_id, command)
        .await
        .map_err(|_| ApiError::BadRequest {
            reason: "deployment not found".to_string(),
        })?;

    Ok(Response::OK(UpdateDeploymentResponse { data: deployment }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::app_state;

    #[tokio::test]
    async fn update_deployment_rejects_invalid_kind() {
        let state = app_state();
        let request = UpdateDeploymentRequest {
            name: None,
            kind: Some("invalid".to_string()),
            version: None,
            status: None,
            namespace: None,
            deployed_at: None,
        };

        let result = update_deployment_handler(
            UpdateDeploymentRoute {
                organisation_id: Uuid::new_v4(),
                deployment_id: Uuid::new_v4(),
            },
            State(state),
            Json(request),
        )
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest { .. })));
    }

    #[tokio::test]
    async fn update_deployment_rejects_invalid_status() {
        let state = app_state();
        let request = UpdateDeploymentRequest {
            name: None,
            kind: None,
            version: None,
            status: Some("bad".to_string()),
            namespace: None,
            deployed_at: None,
        };

        let result = update_deployment_handler(
            UpdateDeploymentRoute {
                organisation_id: Uuid::new_v4(),
                deployment_id: Uuid::new_v4(),
            },
            State(state),
            Json(request),
        )
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest { .. })));
    }

    #[tokio::test]
    async fn update_deployment_rejects_invalid_deployed_at() {
        let state = app_state();
        let request = UpdateDeploymentRequest {
            name: None,
            kind: None,
            version: None,
            status: None,
            namespace: None,
            deployed_at: Some("not-a-date".to_string()),
        };

        let result = update_deployment_handler(
            UpdateDeploymentRoute {
                organisation_id: Uuid::new_v4(),
                deployment_id: Uuid::new_v4(),
            },
            State(state),
            Json(request),
        )
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest { .. })));
    }
}
