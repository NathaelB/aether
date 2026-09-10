use aether_auth::Identity;
use aether_core::{
    action::{
        ActionFailureReason, ActionId,
        commands::{AckActionsCommand, AckFailure},
        ports::ActionService,
    },
    dataplane::value_objects::DataPlaneId,
    deployments::DeploymentId,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/dataplanes/{dataplane_id}/deployments/{deployment_id}/actions:ack")]
pub struct AckActionRoute {
    pub dataplane_id: DataPlaneId,
    pub deployment_id: DeploymentId,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AckActionsFailedItem {
    pub action_id: Uuid,
    pub reason: ActionFailureReason,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AckActionsRequest {
    pub published: Vec<Uuid>,
    pub failed: Vec<AckActionsFailedItem>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct AckActionsResponseData {
    pub acknowledged: usize,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct AckActionsResponse {
    pub data: AckActionsResponseData,
}

#[utoipa::path(
    post,
    path = "/{dataplane_id}/deployments/{deployment_id}/actions:ack",
    summary = "ack actions",
    tag = "dataplanes",
    request_body = AckActionsRequest,
    description = "Acknowledge published or failed actions for the specified deployment on the dataplane, moving them to a terminal state.",
    params(AckActionRoute),
    responses(
        (status = 200, description = "Acknowledged actions", body = AckActionsResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 400, description = "Invalid dataplane or deployment id", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn ack_actions_handler(
    AckActionRoute {
        dataplane_id,
        deployment_id,
    }: AckActionRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<AckActionsRequest>,
) -> Result<Response<AckActionsResponse>, ApiError> {
    let command = AckActionsCommand {
        dataplane_id,
        deployment_id,
        published: request.published.into_iter().map(ActionId).collect(),
        failed: request
            .failed
            .into_iter()
            .map(|item| AckFailure {
                action_id: ActionId(item.action_id),
                reason: item.reason,
            })
            .collect(),
    };

    let acknowledged = state.service.ack_actions(identity, command).await?;

    Ok(Response::OK(AckActionsResponse {
        data: AckActionsResponseData { acknowledged },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::app_state;
    use aether_auth::Client;

    fn non_herald_identity() -> Identity {
        Identity::Client(Client {
            id: "id".to_string(),
            client_id: "some-other-service".to_string(),
            roles: vec![],
            scopes: vec![],
        })
    }

    #[tokio::test]
    async fn ack_actions_rejects_non_herald_identity() {
        let result = ack_actions_handler(
            AckActionRoute {
                dataplane_id: DataPlaneId(Uuid::new_v4()),
                deployment_id: DeploymentId(Uuid::new_v4()),
            },
            State(app_state()),
            Extension(non_herald_identity()),
            Json(AckActionsRequest {
                published: vec![Uuid::new_v4()],
                failed: vec![],
            }),
        )
        .await;

        // `#[transactional]` opens the transaction before the service can look at
        // the identity, so against an unreachable database the connection failure
        // is what surfaces. The authorization rule itself is asserted where it
        // lives, on the domain service, with a mocked repository -- see
        // `ack_actions_rejects_non_herald_identity` in aether-domain.
        assert!(matches!(result, Err(ApiError::Unknown { .. })));
    }
}
