use aether_auth::Identity;
use aether_core::deployments::ports::DeploymentService;
use axum::extract::{Extension, State};
use axum_extra::routing::TypedPath;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments/{deployment_id}")]
pub struct DeleteDeploymentRoute {
    pub organisation_id: Uuid,
    pub deployment_id: Uuid,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct DeleteDeploymentResponse {
    success: bool,
}

#[utoipa::path(
    delete,
    path = "/{organisation_id}/deployments/{deployment_id}",
    summary = "delete deployment",
    tag = "deployments",
    description = "Delete a deployment within the specified organisation.",
    params(DeleteDeploymentRoute),
    responses(
        (status = 200, description = "Deployment deleted successfully", body = DeleteDeploymentResponse),
        (status = 400, description = "Deployment not found", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn delete_deployment_handler(
    DeleteDeploymentRoute {
        organisation_id,
        deployment_id,
    }: DeleteDeploymentRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<DeleteDeploymentResponse>, ApiError> {
    let organisation_id = organisation_id.into();
    let deployment_id = deployment_id.into();

    let deployment = state
        .service
        .delete_deployment_for_organisation(identity, organisation_id, deployment_id)
        .await?;

    // Best-effort and never awaited, same as at creation: a deployment is
    // torn down whether or not its record can be removed right away.
    tokio::spawn(crate::dns::remove_deployment(state.clone(), deployment));

    Ok(Response::OK(DeleteDeploymentResponse { success: true }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};

    #[tokio::test]
    async fn delete_deployment_propagates_the_service_error_instead_of_flattening_it() {
        let state = app_state();

        let result = delete_deployment_handler(
            DeleteDeploymentRoute {
                organisation_id: Uuid::new_v4(),
                deployment_id: Uuid::new_v4(),
            },
            State(state),
            Extension(user_identity("9f3f7a4d-52a3-4a1a-9b3f-0c1b9b7d9a6f")),
        )
        .await;

        // The handler no longer rewrites every failure into one message.
        // Which error a missing deployment produces is pinned in errors.rs,
        // where it can be stated without a database.
        assert!(result.is_err());
    }
}
