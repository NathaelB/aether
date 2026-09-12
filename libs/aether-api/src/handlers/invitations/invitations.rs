use aether_auth::Identity;
use aether_core::organisation::{
    OrganisationId,
    invitation::{Invitation, InvitedEmail},
    ports::InvitationService,
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
#[typed_path("/organisations/{organisation_id}/invitations")]
pub struct InvitationsRoute {
    pub organisation_id: Uuid,
}

#[derive(Deserialize, ToSchema)]
pub struct InviteRequest {
    /// The address the link is written to. Whoever turns up holding it has to
    /// be signed in as this address.
    pub email: String,

    /// What they will hold once they are in. Every role must belong to this
    /// organisation.
    #[serde(default)]
    pub roles: Vec<Uuid>,
}

/// An invitation as anybody may read it back. No secret in it, ever.
#[derive(Serialize, ToSchema, PartialEq)]
pub struct InvitationResponse {
    pub data: Invitation,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ListInvitationsResponse {
    pub data: Vec<Invitation>,
}

/// The one response that carries the secret.
///
/// It exists at this moment and never again: nothing stores it, no listing
/// returns it, and reading the row back gives the hash. Whoever creates the
/// invitation is the only person who will ever see it.
#[derive(Serialize, ToSchema, PartialEq)]
pub struct InviteResponse {
    pub data: Invitation,

    /// Hand this to the person being invited. It is not shown again.
    pub token: String,
}

#[utoipa::path(
    post,
    path = "/{organisation_id}/invitations",
    summary = "invite somebody into an organisation",
    tag = "invitations",
    description = "Returns the secret once, in this response and nowhere else. No mail is sent: the link is handed over by whoever created it.",
    params(InvitationsRoute),
    request_body = InviteRequest,
    responses(
        (status = 201, description = "The invitation, with its secret", body = InviteResponse),
        (status = 400, description = "Not an address, or a role from another organisation", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not invite anybody here", body = ApiError),
        (status = 409, description = "That address is already a member", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn invite_handler(
    InvitationsRoute { organisation_id }: InvitationsRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<InviteRequest>,
) -> Result<Response<InviteResponse>, ApiError> {
    let email = InvitedEmail::parse(&request.email)?;
    let roles = request
        .roles
        .into_iter()
        .map(aether_core::role::RoleId)
        .collect();

    let (invitation, token) = state
        .service
        .invite(identity, OrganisationId(organisation_id), email, roles)
        .await?;

    Ok(Response::Created(InviteResponse {
        data: invitation,
        token: token.expose().to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/invitations",
    summary = "what is outstanding",
    tag = "invitations",
    description = "Every invitation this organisation has issued, with its state. The secret is not in the projection at all.",
    params(InvitationsRoute),
    responses(
        (status = 200, description = "The invitations", body = ListInvitationsResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not see who is in this organisation", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_invitations_handler(
    InvitationsRoute { organisation_id }: InvitationsRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<ListInvitationsResponse>, ApiError> {
    let invitations = state
        .service
        .list_invitations(identity, OrganisationId(organisation_id))
        .await?;

    Ok(Response::OK(ListInvitationsResponse { data: invitations }))
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/invitations/{invitation_id}")]
pub struct InvitationRoute {
    pub organisation_id: Uuid,
    pub invitation_id: Uuid,
}

#[utoipa::path(
    delete,
    path = "/{organisation_id}/invitations/{invitation_id}",
    summary = "cut a link off",
    tag = "invitations",
    description = "Revoking one already accepted cuts the link and leaves the membership alone: putting somebody out is a different act, and it needs a different right.",
    params(InvitationRoute),
    responses(
        (status = 200, description = "The invitation, now revoked", body = InvitationResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not invite anybody here", body = ApiError),
        (status = 404, description = "No such invitation", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn revoke_invitation_handler(
    InvitationRoute {
        organisation_id,
        invitation_id,
    }: InvitationRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<InvitationResponse>, ApiError> {
    let invitation = state
        .service
        .revoke_invitation(
            identity,
            OrganisationId(organisation_id),
            aether_core::organisation::invitation::InvitationId(invitation_id),
        )
        .await?;

    Ok(Response::OK(InvitationResponse { data: invitation }))
}
