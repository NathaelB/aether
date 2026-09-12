use aether_auth::Identity;
use aether_core::deployments::{Deployment, ports::DeploymentService};
use axum::extract::{Extension, State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ListDeploymentsResponse {
    data: Vec<Deployment>,
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments")]
pub struct ListDeploymentsRoute {
    pub organisation_id: Uuid,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/deployments",
    summary = "list deployments",
    tag = "deployments",
    description = "List deployments for the specified organisation.",
    params(ListDeploymentsRoute),
    responses(
        (status = 200, description = "List of deployments", body = ListDeploymentsResponse),
        (status = 400, description = "Invalid organisation id", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    )
)]
pub async fn list_deployments_handler(
    ListDeploymentsRoute { organisation_id }: ListDeploymentsRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<ListDeploymentsResponse>, ApiError> {
    let organisation_id = organisation_id.into();

    let deployments = state
        .service
        .list_deployments_by_organisation(identity, organisation_id)
        .await?;

    Ok(Response::OK(ListDeploymentsResponse { data: deployments }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};

    #[tokio::test]
    async fn list_deployments_maps_service_error() {
        let state = app_state();

        let result = list_deployments_handler(
            ListDeploymentsRoute {
                organisation_id: Uuid::new_v4(),
            },
            State(state),
            Extension(user_identity("9f3f7a4d-52a3-4a1a-9b3f-0c1b9b7d9a6f")),
        )
        .await;

        assert!(matches!(result, Err(ApiError::Unknown { .. })));
    }
}
