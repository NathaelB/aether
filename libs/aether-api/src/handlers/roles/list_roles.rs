use aether_core::role::{Role, ports::RoleService};
use axum::extract::State;
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ListRolesResponse {
    data: Vec<Role>,
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/roles")]
pub struct ListRolesRoute {
    pub organisation_id: Uuid,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/roles",
    summary = "list roles",
    tag = "roles",
    description = "List all roles for the specified organisation.",
    params(ListRolesRoute),
    responses(
        (status = 200, description = "List of roles", body = ListRolesResponse),
        (status = 400, description = "Invalid organisation id", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    )
)]
pub async fn list_roles_handler(
    ListRolesRoute { organisation_id }: ListRolesRoute,
    State(state): State<AppState>,
) -> Result<Response<ListRolesResponse>, ApiError> {
    let organisation_id = organisation_id.into();

    let roles = state
        .service
        .list_roles_by_organisation(organisation_id)
        .await?;

    Ok(Response::OK(ListRolesResponse { data: roles }))
}
