use aether_auth::Identity;
use aether_core::{
    organisation::{OrganisationId, member::Member, ports::MemberService},
    user::UserId,
};
use axum::extract::{Extension, State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/members/{user_id}")]
pub struct MemberRoute {
    pub organisation_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct MemberResponse {
    pub data: Member,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/members/{user_id}",
    summary = "read one member",
    tag = "members",
    description = "Addressed by the user's id rather than the membership's: the caller knows who they are asking about, not which row records it.",
    params(MemberRoute),
    responses(
        (status = 200, description = "The member", body = MemberResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not see who is in this organisation", body = ApiError),
        (status = 404, description = "That person is not in this organisation", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_member_handler(
    MemberRoute {
        organisation_id,
        user_id,
    }: MemberRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<MemberResponse>, ApiError> {
    let member = state
        .service
        .member(identity, OrganisationId(organisation_id), UserId(user_id))
        .await?;

    Ok(Response::OK(MemberResponse { data: member }))
}

#[utoipa::path(
    delete,
    path = "/{organisation_id}/members/{user_id}",
    summary = "put somebody out of an organisation",
    tag = "members",
    description = "The owner cannot be removed, whatever the caller may do: an organisation without its owner is one nobody can recover.",
    params(MemberRoute),
    responses(
        (status = 204, description = "They are no longer a member"),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not remove members", body = ApiError),
        (status = 404, description = "That person is not in this organisation", body = ApiError),
        (status = 409, description = "That person is the owner", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn remove_member_handler(
    MemberRoute {
        organisation_id,
        user_id,
    }: MemberRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<axum::http::StatusCode, ApiError> {
    state
        .service
        .remove_member(identity, OrganisationId(organisation_id), UserId(user_id))
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}
