use aether_auth::Identity;
use aether_core::{
    deployments::DeploymentId,
    logs::{LogLine, LogSessionId, commands::PushLogLinesCommand, ports::LogService},
};
use axum::{Extension, Json, extract::State};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/dataplanes/{dataplane_id}/deployments/{deployment_id}/logs/{session_id}")]
pub struct PushLogsRoute {
    pub dataplane_id: Uuid,
    pub deployment_id: Uuid,
    pub session_id: Uuid,
}

#[derive(Deserialize, ToSchema)]
pub struct PushLogsRequest {
    pub lines: Vec<LogLine>,
    /// Nothing further will be sent for this session.
    #[serde(default)]
    pub done: bool,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct PushLogsResponseData {
    pub relayed: usize,
    /// Whether anybody is still reading. False means stop sending: the reader
    /// closed the page, and nothing further will reach anyone.
    pub listening: bool,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct PushLogsResponse {
    data: PushLogsResponseData,
}

#[utoipa::path(
    post,
    path = "/{dataplane_id}/deployments/{deployment_id}/logs/{session_id}",
    summary = "send log lines for an open session",
    tag = "dataplanes",
    description = "Lines are relayed to whoever opened the session and are never written down. A session nobody is reading any more is accepted and discarded, because the data plane had no way to know the reader left; `listening` says so, and is the signal to stop sending.",
    params(PushLogsRoute),
    request_body = PushLogsRequest,
    responses(
        (status = 200, description = "Relayed", body = PushLogsResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Only a data plane agent may send logs", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn push_logs_handler(
    PushLogsRoute {
        dataplane_id: _,
        deployment_id,
        session_id,
    }: PushLogsRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    Json(request): Json<PushLogsRequest>,
) -> Result<Response<PushLogsResponse>, ApiError> {
    let relayed = request.lines.len();

    let listening = state
        .service
        .push_log_lines(
            identity,
            PushLogLinesCommand {
                session_id: LogSessionId(session_id),
                deployment_id: DeploymentId(deployment_id),
                lines: request.lines,
                done: request.done,
            },
        )
        .await?;

    Ok(Response::OK(PushLogsResponse {
        data: PushLogsResponseData { relayed, listening },
    }))
}
