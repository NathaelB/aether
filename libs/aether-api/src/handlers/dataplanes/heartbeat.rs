use aether_auth::Identity;
use aether_core::dataplane::{ports::DataPlaneService, value_objects::DataPlaneId};
use axum::{Extension, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, Deserialize)]
#[typed_path("/dataplanes/{dataplane_id}/heartbeat")]
pub struct HeartbeatRoute {
    pub dataplane_id: DataPlaneId,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct HeartbeatResponseData {
    /// `false` when no data plane carries this id, so a Herald configured with
    /// a stale one learns it instead of reporting into the void.
    pub recorded: bool,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct HeartbeatResponse {
    data: HeartbeatResponseData,
}

#[utoipa::path(
    post,
    path = "/{dataplane_id}/heartbeat",
    summary = "report that a data plane is alive",
    tag = "dataplanes",
    description = "Records that this data plane's Herald is running. A data plane that \
                   stops reporting is no longer selected for new deployments.",
    responses(
        (status = 200, description = "Heartbeat recorded", body = HeartbeatResponse),
        (status = 400, description = "Invalid dataplane id", body = ApiError),
        (status = 403, description = "Caller is not herald", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    )
)]
pub async fn heartbeat_handler(
    HeartbeatRoute { dataplane_id }: HeartbeatRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<HeartbeatResponse>, ApiError> {
    let recorded = state
        .service
        .record_heartbeat(identity, dataplane_id)
        .await?;

    Ok(Response::OK(HeartbeatResponse {
        data: HeartbeatResponseData { recorded },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::app_state;
    use aether_auth::Client;
    use uuid::Uuid;

    fn non_herald_identity() -> Identity {
        Identity::Client(Client {
            id: "id".to_string(),
            client_id: "some-other-service".to_string(),
            roles: vec![],
            scopes: vec![],
        })
    }

    /// A forged heartbeat would keep a dead cluster receiving deployments, so
    /// the endpoint carries the same rule as claim and ack. The rule itself is
    /// asserted on the domain service; here the transaction opens first, so an
    /// unreachable database is what surfaces.
    #[tokio::test]
    async fn heartbeat_requires_a_service_identity() {
        let result = heartbeat_handler(
            HeartbeatRoute {
                dataplane_id: DataPlaneId(Uuid::new_v4()),
            },
            State(app_state()),
            Extension(non_herald_identity()),
        )
        .await;

        assert!(result.is_err());
    }
}
