use aether_auth::Identity;
use aether_core::{organisation::OrganisationId, role::ports::PermissionProvider};
use axum::extract::{Extension, State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/permissions")]
pub struct MyPermissionsRoute {
    pub organisation_id: Uuid,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct MyPermissions {
    /// The bits the caller holds here, as one mask.
    ///
    /// A number rather than a list of names: the console already has the
    /// catalogue mapping bits to labels, and sending names would be a second
    /// spelling of the same thing for the two to disagree about.
    pub permissions: u64,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct MyPermissionsResponse {
    pub data: MyPermissions,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/permissions",
    summary = "what the caller may do in this organisation",
    tag = "permissions",
    description = "Zero means nothing, which is what somebody outside the organisation gets. The owner's mask carries ADMINISTRATOR, which stands for all of them rather than setting each one.",
    params(MyPermissionsRoute),
    responses(
        (status = 200, description = "The caller's permissions here", body = MyPermissionsResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn my_permissions_handler(
    MyPermissionsRoute { organisation_id }: MyPermissionsRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<MyPermissionsResponse>, ApiError> {
    // Gated on nothing, deliberately. Asking what you may do cannot itself
    // need a permission: somebody granted nothing would be unable to find out
    // they were granted nothing, and a screen would have to guess.
    //
    // It reveals nothing either. The answer is about the caller, and somebody
    // outside gets zero -- which is what every other endpoint already tells
    // them by refusing.
    let permissions = state
        .service
        .permissions_for_organisation(identity, OrganisationId(organisation_id))
        .await?;

    Ok(Response::OK(MyPermissionsResponse {
        data: MyPermissions {
            permissions: permissions.bits(),
        },
    }))
}
