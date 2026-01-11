use aether_core::role::{Role, commands::UpdateRoleCommand, ports::RoleService};
use axum::{Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Deserialize, ToSchema)]
pub struct UpdateRoleRequest {
    pub name: Option<String>,
    pub permissions: Option<u64>,
    pub color: Option<String>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct UpdateRoleResponse {
    data: Role,
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/roles/{role_id}")]
pub struct UpdateRoleRoute {
    pub organisation_id: Uuid,
    pub role_id: Uuid,
}

#[utoipa::path(
    patch,
    path = "/{organisation_id}/roles/{role_id}",
    summary = "update role",
    tag = "roles",
    description = "Update a role within the specified organisation.",
    request_body = UpdateRoleRequest,
    params(UpdateRoleRoute),
    responses(
        (status = 200, description = "Role updated successfully", body = UpdateRoleResponse),
        (status = 400, description = "Invalid request data", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_role_handler(
    UpdateRoleRoute {
        organisation_id,
        role_id,
    }: UpdateRoleRoute,
    State(state): State<AppState>,
    Json(request): Json<UpdateRoleRequest>,
) -> Result<Response<UpdateRoleResponse>, ApiError> {
    let organisation_id = organisation_id.into();
    let role_id = role_id.into();

    let existing_role = state
        .service
        .get_role(role_id)
        .await?
        .ok_or(ApiError::BadRequest {
            reason: "role not found".to_string(),
        })?;

    if existing_role.organisation_id != Some(organisation_id) {
        return Err(ApiError::BadRequest {
            reason: "role not found".to_string(),
        });
    }

    let mut command = UpdateRoleCommand::new();
    if let Some(name) = request.name {
        command = command.with_name(name);
    }
    if let Some(permissions) = request.permissions {
        command = command.with_permissions(permissions);
    }
    if let Some(color) = request.color {
        command = command.with_color(color);
    }

    let role = state.service.update_role(role_id, command).await?;

    Ok(Response::OK(UpdateRoleResponse { data: role }))
}
