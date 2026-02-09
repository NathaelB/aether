use aether_auth::Identity;
use aether_core::{
    dataplane::{
        ports::DataPlaneService,
        value_objects::{DataPlaneId, ListDataPlaneDeploymentsCommand},
    },
    deployments::Deployment,
};
use axum::{
    Extension,
    extract::{Query, State},
};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ListDeploymentsForDataPlaneResponse {
    pub data: Vec<Deployment>,
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/dataplanes/{dataplane_id}/deployments")]
pub struct ListDeploymentsForDataPlaneRoute {
    pub dataplane_id: DataPlaneId,
}

#[derive(Deserialize, IntoParams)]
pub struct ListDeploymentsForDataPlaneQueryParams {
    pub shard_index: Option<usize>,
    pub shard_count: Option<usize>,
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}

#[utoipa::path(
    get,
    path = "/{dataplane_id}/deployments",
    summary = "list deployments for dataplane",
    tag = "dataplanes",
    description = "List deployments associated with the specified dataplane.",
    params(ListDeploymentsForDataPlaneQueryParams),
    responses(
        (status = 200, description = "List of deployments for dataplane", body = ListDeploymentsForDataPlaneResponse),
        (status = 400, description = "Invalid dataplane id", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    )
)]
pub async fn list_deployments_for_dataplane_handler(
    ListDeploymentsForDataPlaneRoute { dataplane_id }: ListDeploymentsForDataPlaneRoute,
    Query(query): Query<ListDeploymentsForDataPlaneQueryParams>,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<ListDeploymentsForDataPlaneResponse>, ApiError> {
    if let Some(cursor) = &query.cursor {
        Uuid::parse_str(cursor).map_err(|_| ApiError::BadRequest {
            reason: "cursor must be a valid UUID".to_string(),
        })?;
    }

    let command = ListDataPlaneDeploymentsCommand::new(
        query.shard_index,
        query.shard_count,
        query.limit,
        query.cursor,
    )
    .map_err(|reason| ApiError::BadRequest { reason })?;

    let deployments = state
        .service
        .get_deployments_in_dataplane(identity, dataplane_id, command)
        .await?;

    Ok(Response::OK(ListDeploymentsForDataPlaneResponse {
        data: deployments,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::app_state;
    use aether_auth::{Client, Identity};

    #[tokio::test]
    async fn list_deployments_for_dataplane_rejects_invalid_shard_params() {
        let result = list_deployments_for_dataplane_handler(
            ListDeploymentsForDataPlaneRoute {
                dataplane_id: DataPlaneId(Uuid::new_v4()),
            },
            Query(ListDeploymentsForDataPlaneQueryParams {
                shard_index: Some(4),
                shard_count: Some(4),
                limit: Some(10),
                cursor: None,
            }),
            State(app_state()),
            Extension(Identity::Client(Client {
                id: "id".to_string(),
                client_id: "herald-service".to_string(),
                roles: vec![],
                scopes: vec![],
            })),
        )
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest { .. })));
    }

    #[tokio::test]
    async fn list_deployments_for_dataplane_rejects_invalid_cursor() {
        let result = list_deployments_for_dataplane_handler(
            ListDeploymentsForDataPlaneRoute {
                dataplane_id: DataPlaneId(Uuid::new_v4()),
            },
            Query(ListDeploymentsForDataPlaneQueryParams {
                shard_index: Some(0),
                shard_count: Some(1),
                limit: Some(10),
                cursor: Some("not-a-uuid".to_string()),
            }),
            State(app_state()),
            Extension(Identity::Client(Client {
                id: "id".to_string(),
                client_id: "herald-service".to_string(),
                roles: vec![],
                scopes: vec![],
            })),
        )
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest { .. })));
    }
}
