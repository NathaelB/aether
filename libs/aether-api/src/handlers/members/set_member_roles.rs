use aether_auth::Identity;
use aether_core::{
    organisation::{OrganisationId, member::Member, ports::MemberService},
    role::RoleId,
    user::UserId,
};
use axum::{
    Json,
    extract::{Extension, State},
};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/members/{user_id}/roles")]
pub struct MemberRolesRoute {
    pub organisation_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Deserialize, ToSchema)]
pub struct SetMemberRolesRequest {
    /// The roles this member will hold, replacing whatever they hold now.
    ///
    /// An empty list leaves them in the organisation holding nothing, which
    /// is a state somebody can choose. It is not the same as removing them.
    #[serde(default)]
    pub roles: Vec<Uuid>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct SetMemberRolesResponse {
    pub data: Member,
}

#[utoipa::path(
    put,
    path = "/{organisation_id}/members/{user_id}/roles",
    summary = "replace the roles a member holds",
    tag = "members",
    description = "Replaces the whole set rather than adding or removing one: two people editing through add and remove calls converge on a set neither of them wrote. Every role must belong to this organisation.",
    params(MemberRolesRoute),
    request_body = SetMemberRolesRequest,
    responses(
        (status = 200, description = "The member with the roles they now hold", body = SetMemberRolesResponse),
        (status = 400, description = "One of the roles belongs to another organisation", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not change what members may do", body = ApiError),
        (status = 404, description = "That person is not in this organisation", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn set_member_roles_handler(
    MemberRolesRoute {
        organisation_id,
        user_id,
    }: MemberRolesRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<SetMemberRolesRequest>,
) -> Result<Response<SetMemberRolesResponse>, ApiError> {
    let roles = request.roles.into_iter().map(RoleId).collect();

    let member = state
        .service
        .set_member_roles(
            identity,
            OrganisationId(organisation_id),
            UserId(user_id),
            roles,
        )
        .await?;

    Ok(Response::OK(SetMemberRolesResponse { data: member }))
}
