use aether_auth::Identity;
use aether_core::{
    deployments::{
        Deployment, DeploymentKind, DeploymentName, DeploymentStatus, DeploymentVersion,
        commands::CreateDeploymentCommand, ports::DeploymentService,
    },
    user::UserId,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct CreateDeploymentRequest {
    pub name: String,
    pub kind: String,
    pub version: String,
    pub status: Option<String>,
    pub namespace: String,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct CreateDeploymentResponse {
    data: Deployment,
}

struct ParsedCreateDeploymentRequest {
    name: String,
    kind: DeploymentKind,
    version: String,
    status: DeploymentStatus,
    namespace: String,
}

impl TryFrom<CreateDeploymentRequest> for ParsedCreateDeploymentRequest {
    type Error = ApiError;

    fn try_from(request: CreateDeploymentRequest) -> Result<Self, Self::Error> {
        let kind =
            DeploymentKind::try_from(request.kind.as_str()).map_err(|e| ApiError::BadRequest {
                reason: e.to_string(),
            })?;

        let status = match request.status {
            Some(status) => {
                DeploymentStatus::try_from(status.as_str()).map_err(|e| ApiError::BadRequest {
                    reason: e.to_string(),
                })?
            }
            None => DeploymentStatus::Pending,
        };

        Ok(Self {
            name: request.name,
            kind,
            version: request.version,
            status,
            namespace: request.namespace,
        })
    }
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments")]
pub struct CreateDeploymentRoute {
    pub organisation_id: Uuid,
}

#[utoipa::path(
    post,
    path = "/{organisation_id}/deployments",
    summary = "create deployment",
    tag = "deployments",
    description = "Create a deployment within the specified organisation.",
    request_body = CreateDeploymentRequest,
    params(CreateDeploymentRoute),
    responses(
        (status = 201, description = "Deployment created successfully", body = CreateDeploymentResponse),
        (status = 400, description = "Invalid request data", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_deployment_handler(
    CreateDeploymentRoute { organisation_id }: CreateDeploymentRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<CreateDeploymentRequest>,
) -> Result<Response<CreateDeploymentResponse>, ApiError> {
    let organisation_id = organisation_id.into();
    let created_by =
        identity
            .id()
            .parse::<UserId>()
            .map_err(|e| ApiError::InternalServerError {
                reason: e.to_string(),
            })?;
    let parsed = ParsedCreateDeploymentRequest::try_from(request)?;

    let command = CreateDeploymentCommand::new(
        organisation_id,
        DeploymentName(parsed.name),
        parsed.kind,
        DeploymentVersion(parsed.version),
        parsed.status,
        parsed.namespace,
        created_by,
    );

    let deployment = state.service.create_deployment(command).await?;

    Ok(Response::Created(CreateDeploymentResponse {
        data: deployment,
    }))
}
