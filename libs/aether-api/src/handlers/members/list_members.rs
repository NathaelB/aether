use aether_auth::Identity;
use aether_core::organisation::{OrganisationId, member::Member, ports::MemberService};
use axum::extract::{Extension, State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/members")]
pub struct ListMembersRoute {
    pub organisation_id: Uuid,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ListMembersResponse {
    pub data: Vec<Member>,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/members",
    summary = "list who is in an organisation",
    tag = "members",
    description = "Each member with the roles they hold here. The owner appears like anybody else; what makes them the owner is the organisation, not a role.",
    params(ListMembersRoute),
    responses(
        (status = 200, description = "The members of this organisation", body = ListMembersResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not see who is in this organisation", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_members_handler(
    ListMembersRoute { organisation_id }: ListMembersRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<ListMembersResponse>, ApiError> {
    let members = state
        .service
        .list_members(identity, OrganisationId(organisation_id))
        .await?;

    Ok(Response::OK(ListMembersResponse { data: members }))
}
