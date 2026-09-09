use aether_auth::Identity;
use aether_core::{
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
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath)]
#[typed_path("/dataplanes")]
pub struct CreateDataPlaneRoute;

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
        (status = 200, description = "Created dataplane", body = DataPlane),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    )
)]
pub async fn create_dataplane_handler(
    _: CreateDataPlaneRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<CreateDataPlaneRequest>,
) -> Result<Response<DataPlane>, ApiError> {
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

    Ok(Response::Created(dataplane))
}
