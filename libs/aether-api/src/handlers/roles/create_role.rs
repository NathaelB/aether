use aether_auth::Identity;
use aether_core::role::{Role, commands::CreateRoleCommand, ports::RoleService};

use axum::{
    Json,
    extract::{Extension, State},
};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct CreateRoleRequest {
    pub name: String,
    pub permissions: u64,
    pub color: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, PartialEq)]
pub struct CreateRoleResponse {
    data: Role,
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/roles")]
pub struct CreateRoleRoute {
    pub organisation_id: Uuid,
}

#[utoipa::path(
    post,
    path = "/{organisation_id}/roles",
    summary = "create role",
    tag = "roles",
    description = "Create a role within the specified organisation.",
    request_body = CreateRoleRequest,
    params(CreateRoleRoute),
    responses(
        (status = 201, description = "Role created successfully", body = CreateRoleResponse),
        (status = 400, description = "Invalid request data", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_role_handler(
    CreateRoleRoute { organisation_id }: CreateRoleRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<CreateRoleRequest>,
) -> Result<Response<CreateRoleResponse>, ApiError> {
    let organisation_id = organisation_id.into();
    let mut command = CreateRoleCommand::new(request.name, request.permissions)
        .with_organisation_id(organisation_id);

    if let Some(color) = request.color {
        command = command.with_color(color);
    }

    let role = state.service.create_role(identity, command).await?;

    Ok(Response::Created(CreateRoleResponse { data: role }))
}
