use aether_auth::Identity;
use aether_core::{
    dataplane::value_objects::DataPlaneId,
    dataplane::{
        entities::DataPlane,
        ports::DataPlaneService,
        value_objects::{
            Capacity, CreateDataplaneCommand, DataPlaneAllocation, DataPlaneMode, Region,
        },
    },
    organisation::OrganisationId,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath)]
#[typed_path("/dataplanes")]
pub struct CreateDataPlaneRoute;

/// A registered data plane, and the secret its Herald authenticates with.
///
/// The secret is here and nowhere else. Nothing stores it, so this response is
/// the only time it can be read -- an installation that loses it re-issues,
/// which is also what it does when one leaks.
#[derive(Serialize, ToSchema, PartialEq)]
pub struct RegisteredDataPlaneResponse {
    #[serde(flatten)]
    pub dataplane: DataPlane,

    /// The client this data plane's Herald authenticates as, for the chart.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub herald_client_id: Option<String>,

    /// Absent when this installation has no realm administrator configured and
    /// therefore cannot give a cluster an identity of its own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub herald_secret: Option<String>,
}

impl From<aether_core::dataplane::herald_identity::RegisteredDataPlane>
    for RegisteredDataPlaneResponse
{
    fn from(registered: aether_core::dataplane::herald_identity::RegisteredDataPlane) -> Self {
        Self {
            herald_client_id: registered
                .dataplane
                .herald
                .as_ref()
                .map(|herald| herald.client_id.clone()),
            herald_secret: registered.herald_secret,
            dataplane: registered.dataplane,
        }
    }
}

#[derive(Deserialize, ToSchema)]
pub struct CreateDataPlaneRequest {
    pub mode: DataPlaneMode,
    /// Required for `dedicated`, rejected for `shared`. A dedicated data plane
    /// with no owner is one nothing can be placed on; a shared one with an
    /// owner is a contradiction.
    pub organisation_id: Option<OrganisationId>,
    pub region: Region,
    pub capacity: Capacity,
}

#[utoipa::path(
    post,
    path = "",
    summary = "create dataplane",
    tag = "dataplanes",
    request_body = CreateDataPlaneRequest,
    description = "Create a new dataplane with the specified configuration.",
    responses(
        (status = 201, description = "The data plane, and the secret its Herald authenticates with", body = RegisteredDataPlaneResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_dataplane_handler(
    _: CreateDataPlaneRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<CreateDataPlaneRequest>,
) -> Result<Response<RegisteredDataPlaneResponse>, ApiError> {
    let allocation = match (request.mode, request.organisation_id) {
        (DataPlaneMode::Shared, None) => DataPlaneAllocation::Shared,
        (DataPlaneMode::Dedicated, Some(organisation_id)) => {
            DataPlaneAllocation::Dedicated { organisation_id }
        }
        (DataPlaneMode::Dedicated, None) => {
            return Err(ApiError::BadRequest {
                reason: "a dedicated data plane needs an organisation_id".to_string(),
            });
        }
        (DataPlaneMode::Shared, Some(_)) => {
            return Err(ApiError::BadRequest {
                reason: "a shared data plane cannot belong to an organisation".to_string(),
            });
        }
    };

    let dataplane = state
        .service
        .create_dataplane(
            identity,
            CreateDataplaneCommand {
                capacity: request.capacity,
                allocation,
                region: request.region,
            },
        )
        .await?;

    Ok(Response::Created(dataplane.into()))
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/dataplanes/{dataplane_id}/credential")]
pub struct HeraldCredentialRoute {
    pub dataplane_id: DataPlaneId,
}

#[utoipa::path(
    post,
    path = "/{dataplane_id}/credential",
    summary = "issue a data plane a new credential",
    tag = "dataplanes",
    description = "Replaces what this data plane's Herald authenticates with. The previous \
                   secret stops working at once, which is the point: one nobody can \
                   invalidate has to be assumed still in somebody's hands.",
    params(HeraldCredentialRoute),
    responses(
        (status = 201, description = "The data plane, and its new secret", body = RegisteredDataPlaneResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "This needs the operate_fleet right", body = ApiError),
        (status = 404, description = "No such data plane", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn reissue_herald_credential_handler(
    HeraldCredentialRoute { dataplane_id }: HeraldCredentialRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<RegisteredDataPlaneResponse>, ApiError> {
    let registered = state
        .service
        .reissue_herald_credential(identity, dataplane_id)
        .await?;

    Ok(Response::Created(registered.into()))
}
