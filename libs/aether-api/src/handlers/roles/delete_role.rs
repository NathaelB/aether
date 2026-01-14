use aether_auth::Identity;
use aether_core::role::ports::RoleService;
use axum::extract::{Extension, State};
use axum_extra::routing::TypedPath;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/roles/{role_id}")]
pub struct DeleteRoleRoute {
    pub organisation_id: Uuid,
    pub role_id: Uuid,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct DeleteRoleResponse {
    success: bool,
}

#[utoipa::path(
    delete,
    path = "/{organisation_id}/roles/{role_id}",
    summary = "delete role",
    tag = "roles",
    description = "Delete a role within the specified organisation.",
    params(DeleteRoleRoute),
    responses(
        (status = 200, description = "Role deleted successfully", body = DeleteRoleResponse),
        (status = 400, description = "Role not found", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn delete_role_handler(
    DeleteRoleRoute {
        organisation_id,
        role_id,
    }: DeleteRoleRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<DeleteRoleResponse>, ApiError> {
    let organisation_id = organisation_id.into();
    let role_id = role_id.into();

    state
        .service
        .delete_role(identity, organisation_id, role_id)
        .await?;

    Ok(Response::OK(DeleteRoleResponse { success: true }))
}
