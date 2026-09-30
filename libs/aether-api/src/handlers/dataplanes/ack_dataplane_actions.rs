use aether_auth::Identity;
use aether_core::{
    action::{
        ActionId, ActionScope,
        commands::{AckActionsCommand, AckFailure},
    },
    dataplane::value_objects::DataPlaneId,
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::Deserialize;
use utoipa::IntoParams;

use super::ack_actions::{AckActionsRequest, AckActionsResponse, AckActionsResponseData};
use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/dataplanes/{dataplane_id}/actions:ack")]
pub struct AckDataPlaneActionRoute {
    pub dataplane_id: DataPlaneId,
}

#[utoipa::path(
    post,
    path = "/{dataplane_id}/actions:ack",
    summary = "ack data plane actions",
    tag = "dataplanes",
    request_body = AckActionsRequest,
    description = "Acknowledge published or failed actions addressed to the data plane itself, the ones that belong to no deployment.",
    params(AckDataPlaneActionRoute),
    responses(
        (status = 200, description = "Acknowledged actions", body = AckActionsResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 400, description = "Invalid dataplane id", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn ack_dataplane_actions_handler(
    AckDataPlaneActionRoute { dataplane_id }: AckDataPlaneActionRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<AckActionsRequest>,
) -> Result<Response<AckActionsResponse>, ApiError> {
    let command = AckActionsCommand {
        dataplane_id,
        scope: ActionScope::DataPlane(dataplane_id),
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
    use uuid::Uuid;

    #[tokio::test]
    async fn ack_dataplane_actions_rejects_non_herald_identity() {
        let identity = Identity::Client(Client {
            id: "id".to_string(),
            client_id: "some-other-service".to_string(),
            roles: vec![],
            scopes: vec![],
        });

        let result = ack_dataplane_actions_handler(
            AckDataPlaneActionRoute {
                dataplane_id: DataPlaneId(Uuid::new_v4()),
            },
            State(app_state()),
            Extension(identity),
            Json(AckActionsRequest {
                published: vec![Uuid::new_v4()],
                failed: vec![],
            }),
        )
        .await;

        assert!(matches!(result, Err(ApiError::Unknown { .. })));
    }
}
