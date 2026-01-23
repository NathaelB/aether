use aether_auth::Identity;
use aether_core::organisation::{Organisation, ports::OrganisationService};
use axum::{Extension, extract::State};
use axum_extra::routing::TypedPath;
use serde::Serialize;
use utoipa::ToSchema;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Serialize, ToSchema, PartialEq)]
pub struct GetUserOrganisationsResponse {
    data: Vec<Organisation>,
}

#[derive(TypedPath)]
#[typed_path("/users/@me/organisations")]
pub struct GetUserOrganisationsRoute;

#[utoipa::path(
    get,
    path = "/@me/organisations",
    summary = "get user's organisations",
    description = "Retrieve a list of organisations the authenticated user is a member of.",
    responses(
        (status = 200, description = "List of user's organisations", body = GetUserOrganisationsResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "users"
)]
pub async fn get_user_organisations_handler(
    _: GetUserOrganisationsRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<GetUserOrganisationsResponse>, ApiError> {
    let orgs = state.service.get_organisations_by_member(identity).await?;

    Ok(Response::OK(GetUserOrganisationsResponse { data: orgs }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};

    #[tokio::test]
    async fn get_user_organisations_maps_service_error() {
        let state = app_state();
        let identity = user_identity("user-123");

        let result = get_user_organisations_handler(
            GetUserOrganisationsRoute,
            State(state),
            Extension(identity),
        )
        .await;

        assert!(matches!(result, Err(ApiError::Unknown { .. })));
    }
}
