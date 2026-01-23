use aether_auth::Identity;
use aether_core::role::{Role, ports::RoleService};
use axum::extract::{Extension, State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Serialize, ToSchema, PartialEq)]
pub struct GetRoleResponse {
    data: Role,
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/roles/{role_id}")]
pub struct GetRoleRoute {
    pub organisation_id: Uuid,
    pub role_id: Uuid,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/roles/{role_id}",
    summary = "get role",
    tag = "roles",
    description = "Retrieve a role within the specified organisation.",
    params(GetRoleRoute),
    responses(
        (status = 200, description = "Role details", body = GetRoleResponse),
        (status = 400, description = "Role not found", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_role_handler(
    GetRoleRoute {
        organisation_id,
        role_id,
    }: GetRoleRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<GetRoleResponse>, ApiError> {
    let organisation_id = organisation_id.into();
    let role_id = role_id.into();

    let role = state
        .service
        .get_role(identity, organisation_id, role_id)
        .await?
        .ok_or(ApiError::BadRequest {
            reason: "role not found".to_string(),
        })?;

    Ok(Response::OK(GetRoleResponse { data: role }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};

    #[tokio::test]
    async fn get_role_maps_service_error() {
        let state = app_state();
        let identity = user_identity("user-123");

        let result = get_role_handler(
            GetRoleRoute {
                organisation_id: Uuid::new_v4(),
                role_id: Uuid::new_v4(),
            },
            State(state),
            Extension(identity),
        )
        .await;

        assert!(matches!(result, Err(ApiError::Forbidden { .. })));
    }
}
