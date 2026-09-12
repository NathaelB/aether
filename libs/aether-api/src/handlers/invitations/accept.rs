use aether_auth::Identity;
use aether_core::organisation::{
    invitation::InvitationToken, member::Member, ports::InvitationService,
};
use axum::{
    Json,
    extract::{Extension, State},
};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::ApiError, response::Response, state::AppState};

/// Outside any organisation, because whoever is accepting is not in one yet.
#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/invitations/accept")]
pub struct AcceptInvitationRoute {}

#[derive(Deserialize, ToSchema)]
pub struct AcceptInvitationRequest {
    /// The secret from the link.
    ///
    /// In the body rather than the path: a secret in a URL is written to
    /// every access log it passes and travels in the Referer header of
    /// whatever the page loads next.
    pub token: String,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct AcceptInvitationResponse {
    pub data: Member,
}

#[utoipa::path(
    post,
    path = "/invitations/accept",
    summary = "walk through an invitation",
    tag = "invitations",
    description = "Not gated on any permission: the invitation is the authorisation. It is gated on the signed-in account being the address the invitation was written to.",
    request_body = AcceptInvitationRequest,
    responses(
        (status = 200, description = "The membership it created", body = AcceptInvitationResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "This invitation was written to a different address", body = ApiError),
        (status = 404, description = "No such invitation", body = ApiError),
        (status = 409, description = "It expired, was revoked, or has already been accepted", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn accept_invitation_handler(
    _: AcceptInvitationRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<AcceptInvitationRequest>,
) -> Result<Response<AcceptInvitationResponse>, ApiError> {
    let token = InvitationToken::parse(&request.token)?;

    let member = state.service.accept_invitation(identity, token).await?;

    Ok(Response::OK(AcceptInvitationResponse { data: member }))
}
