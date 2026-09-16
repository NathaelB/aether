use aether_auth::Identity;
use aether_core::dataplane::{
    entities::DataPlane,
    ports::DataPlaneService,
    value_objects::{DataPlaneId, ServiceIntent},
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/dataplanes/{dataplane_id}/service")]
pub struct SetServiceRoute {
    pub dataplane_id: DataPlaneId,
}

#[derive(Deserialize, ToSchema)]
pub struct SetServiceRequest {
    /// `draining`, `disabled` or `in_service`.
    ///
    /// Three, where the status has five. `provisioning`, `active` and
    /// `failed` are what the system observed rather than what anybody chose,
    /// so there is no way to ask for them here.
    pub service: ServiceIntent,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct DataPlaneResponse {
    pub data: DataPlane,
}

#[utoipa::path(
    put,
    path = "/{dataplane_id}/service",
    summary = "take a data plane out of service, or put it back",
    tag = "dataplanes",
    description = "Draining and disabling both stop placement and leave every deployment already \
                   on the cluster running, which is what makes them the way to empty a machine \
                   before giving it up. Returning one to service puts it back on the path it was \
                   on: active if its Herald has ever reported, provisioning if it has not, so \
                   that `active` never means anything other than a cluster that answered.",
    params(SetServiceRoute),
    request_body = SetServiceRequest,
    responses(
        (status = 200, description = "The data plane, in its new state", body = DataPlaneResponse),
        (status = 400, description = "No such service state", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Changing the fleet is a separate right", body = ApiError),
        (status = 404, description = "No such data plane", body = ApiError),
        (status = 409, description = "A failed data plane cannot be returned to service", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn set_service_handler(
    SetServiceRoute { dataplane_id }: SetServiceRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<SetServiceRequest>,
) -> Result<Response<DataPlaneResponse>, ApiError> {
    let dataplane = state
        .service
        .set_dataplane_service(identity, dataplane_id, request.service)
        .await?;

    Ok(Response::OK(DataPlaneResponse { data: dataplane }))
}
