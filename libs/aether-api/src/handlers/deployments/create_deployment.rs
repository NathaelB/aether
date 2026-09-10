use aether_auth::Identity;
use aether_core::{
    dataplane::value_objects::{DataPlaneMode, DeploymentResources, Region},
    deployments::{
        Deployment, DeploymentKind, DeploymentName, DeploymentStatus,
        commands::CreateDeploymentCommand, ports::DeploymentService,
    },
    user::UserId,
    version::Version,
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
    /// Where to run this. Omitting it uses the control plane's configured
    /// default region; a region that *is* named is never substituted.
    pub region: Option<String>,
    /// `shared` (the default) or `dedicated`.
    pub mode: Option<String>,
    /// CPU in millicores. Defaults with the rest of the sizing.
    pub cpu_millis: Option<u32>,
    pub memory_mib: Option<u32>,
    pub storage_gib: Option<u32>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct CreateDeploymentResponse {
    data: Deployment,
}

struct ParsedCreateDeploymentRequest {
    name: String,
    kind: DeploymentKind,
    version: Version,
    status: DeploymentStatus,
    namespace: String,
    region: Region,
    mode: DataPlaneMode,
    resources: DeploymentResources,
}

impl ParsedCreateDeploymentRequest {
    fn parse(request: CreateDeploymentRequest, default_region: &str) -> Result<Self, ApiError> {
        let kind =
            DeploymentKind::try_from(request.kind.as_str()).map_err(|e| ApiError::BadRequest {
                reason: e.to_string(),
            })?;

        let version = Version::parse(&request.version).map_err(|e| ApiError::BadRequest {
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

        let region = match request.region.as_deref().map(str::trim) {
            Some(region) if !region.is_empty() => region.to_string(),
            Some(_) => {
                return Err(ApiError::BadRequest {
                    reason: "region must not be empty when provided".to_string(),
                });
            }
            None => default_region.to_string(),
        };

        let mode = match request.mode.as_deref() {
            None | Some("shared") => DataPlaneMode::Shared,
            Some("dedicated") => DataPlaneMode::Dedicated,
            Some(other) => {
                return Err(ApiError::BadRequest {
                    reason: format!(
                        "unknown deployment mode '{other}', expected shared or dedicated"
                    ),
                });
            }
        };

        // All or nothing: a request that sets CPU but not storage is more
        // likely a mistake than an intent, and silently completing it from a
        // default would size a database nobody chose.
        let resources = match (request.cpu_millis, request.memory_mib, request.storage_gib) {
            (None, None, None) => DeploymentResources::DEFAULT,
            (Some(cpu), Some(memory), Some(storage)) => {
                DeploymentResources::new(cpu, memory, storage).map_err(|e| {
                    ApiError::BadRequest {
                        reason: e.to_string(),
                    }
                })?
            }
            _ => {
                return Err(ApiError::BadRequest {
                    reason: "set cpu_millis, memory_mib and storage_gib together, or none of them"
                        .to_string(),
                });
            }
        };

        Ok(Self {
            name: request.name,
            kind,
            version,
            status,
            namespace: request.namespace,
            region: Region::new(region),
            mode,
            resources,
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
    let parsed =
        ParsedCreateDeploymentRequest::parse(request, &state.args.dataplane.default_region)?;

    let command = CreateDeploymentCommand::new(
        organisation_id,
        DeploymentName(parsed.name),
        parsed.kind,
        parsed.version,
        parsed.status,
        parsed.namespace,
        created_by,
        parsed.region,
        parsed.mode,
        parsed.resources,
    );

    let deployment = state.service.create_deployment(command).await?;

    Ok(Response::Created(CreateDeploymentResponse {
        data: deployment,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};

    #[tokio::test]
    async fn create_deployment_rejects_invalid_kind() {
        let state = app_state();
        let identity = user_identity("9f3f7a4d-52a3-4a1a-9b3f-0c1b9b7d9a6f");
        let request = CreateDeploymentRequest {
            name: "deployment".to_string(),
            kind: "invalid".to_string(),
            version: "1.0.0".to_string(),
            status: None,
            region: Some("fr-par".to_string()),
            mode: None,
            cpu_millis: None,
            memory_mib: None,
            storage_gib: None,
            namespace: "default".to_string(),
        };

        let result = create_deployment_handler(
            CreateDeploymentRoute {
                organisation_id: Uuid::new_v4(),
            },
            State(state),
            Extension(identity),
            Json(request),
        )
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest { .. })));
    }

    #[tokio::test]
    async fn create_deployment_rejects_invalid_status() {
        let state = app_state();
        let identity = user_identity("9f3f7a4d-52a3-4a1a-9b3f-0c1b9b7d9a6f");
        let request = CreateDeploymentRequest {
            name: "deployment".to_string(),
            kind: "keycloak".to_string(),
            version: "1.0.0".to_string(),
            region: Some("fr-par".to_string()),
            mode: None,
            cpu_millis: None,
            memory_mib: None,
            storage_gib: None,
            status: Some("bad".to_string()),
            namespace: "default".to_string(),
        };

        let result = create_deployment_handler(
            CreateDeploymentRoute {
                organisation_id: Uuid::new_v4(),
            },
            State(state),
            Extension(identity),
            Json(request),
        )
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest { .. })));
    }

    #[tokio::test]
    async fn create_deployment_rejects_invalid_identity() {
        let state = app_state();
        let identity = user_identity("not-a-uuid");
        let request = CreateDeploymentRequest {
            name: "deployment".to_string(),
            kind: "keycloak".to_string(),
            version: "1.0.0".to_string(),
            status: None,
            region: Some("fr-par".to_string()),
            mode: None,
            cpu_millis: None,
            memory_mib: None,
            storage_gib: None,
            namespace: "default".to_string(),
        };

        let result = create_deployment_handler(
            CreateDeploymentRoute {
                organisation_id: Uuid::new_v4(),
            },
            State(state),
            Extension(identity),
            Json(request),
        )
        .await;

        assert!(matches!(result, Err(ApiError::InternalServerError { .. })));
    }

    #[test]
    fn parsed_request_defaults_status() {
        let request = CreateDeploymentRequest {
            name: "deployment".to_string(),
            kind: "keycloak".to_string(),
            version: "1.0.0".to_string(),
            status: None,
            region: Some("fr-par".to_string()),
            mode: None,
            cpu_millis: None,
            memory_mib: None,
            storage_gib: None,
            namespace: "default".to_string(),
        };

        let parsed = ParsedCreateDeploymentRequest::parse(request, "local").unwrap();
        assert_eq!(parsed.status, DeploymentStatus::Pending);
    }

    /// A request that names a region gets that region, whatever the control
    /// plane's default is. Substituting one would put a deployment somewhere
    /// nobody asked for.
    #[test]
    fn a_named_region_is_never_substituted_by_the_default() {
        let request = CreateDeploymentRequest {
            name: "deployment".to_string(),
            kind: "keycloak".to_string(),
            version: "1.0.0".to_string(),
            status: None,
            region: Some("eu-west".to_string()),
            mode: None,
            cpu_millis: None,
            memory_mib: None,
            storage_gib: None,
            namespace: "default".to_string(),
        };

        let parsed = ParsedCreateDeploymentRequest::parse(request, "fr-par").unwrap();

        assert_eq!(parsed.region.as_str(), "eu-west");
    }

    /// Omitting the region falls back to configuration, not to a constant.
    #[test]
    fn an_absent_region_falls_back_to_the_configured_default() {
        let request = CreateDeploymentRequest {
            name: "deployment".to_string(),
            kind: "keycloak".to_string(),
            version: "1.0.0".to_string(),
            status: None,
            region: None,
            mode: None,
            cpu_millis: None,
            memory_mib: None,
            storage_gib: None,
            namespace: "default".to_string(),
        };

        let parsed = ParsedCreateDeploymentRequest::parse(request, "fr-par").unwrap();

        assert_eq!(parsed.region.as_str(), "fr-par");
        assert_eq!(parsed.mode, DataPlaneMode::Shared, "shared unless asked");
    }

    #[test]
    fn an_unknown_mode_is_rejected_rather_than_defaulted() {
        let request = CreateDeploymentRequest {
            name: "deployment".to_string(),
            kind: "keycloak".to_string(),
            version: "1.0.0".to_string(),
            status: None,
            region: None,
            mode: Some("isolated".to_string()),
            namespace: "default".to_string(),
            cpu_millis: None,
            memory_mib: None,
            storage_gib: None,
        };

        let result = ParsedCreateDeploymentRequest::parse(request, "fr-par");

        assert!(matches!(result, Err(ApiError::BadRequest { .. })));
    }
}
