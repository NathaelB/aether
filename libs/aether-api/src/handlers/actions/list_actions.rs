use aether_core::{
    action::{Action, ActionCursor, commands::FetchActionsCommand, ports::ActionService},
    deployments::ports::DeploymentService,
    organisation::OrganisationId,
};
use axum::extract::{Query, State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, handlers::default_limit, response::Response, state::AppState};

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ListActionsResponse {
    data: Vec<Action>,
    next_cursor: Option<String>,
}

#[derive(Deserialize, IntoParams)]
pub struct ListActionsQuery {
    cursor: Option<String>,

    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments/{deployment_id}/actions")]
pub struct ListActionsRoute {
    pub organisation_id: Uuid,
    pub deployment_id: Uuid,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/deployments/{deployment_id}/actions",
    summary = "list actions",
    tag = "actions",
    description = "List actions for a deployment within the specified organisation.",
    params(ListActionsRoute, ListActionsQuery),
    responses(
        (status = 200, description = "List of actions", body = ListActionsResponse),
        (status = 400, description = "Invalid query parameters", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    )
)]
pub async fn list_actions_handler(
    ListActionsRoute {
        organisation_id,
        deployment_id,
    }: ListActionsRoute,
    Query(query): Query<ListActionsQuery>,
    State(state): State<AppState>,
) -> Result<Response<ListActionsResponse>, ApiError> {
    let organisation_id = OrganisationId(organisation_id);
    let deployment_id = deployment_id.into();

    state
        .service
        .get_deployment_for_organisation(organisation_id, deployment_id)
        .await?;

    let mut command = FetchActionsCommand::new(deployment_id, query.limit);
    if let Some(cursor) = query.cursor {
        command = command.with_cursor(ActionCursor::new(cursor));
    }

    let batch = state.service.fetch_actions(command).await?;

    Ok(Response::OK(ListActionsResponse {
        data: batch.actions,
        next_cursor: batch.next_cursor.map(|cursor| cursor.0),
    }))
}
