use aether_auth::Identity;
use aether_core::{
    deployments::DeploymentId,
    logs::{LogLine, LogSessionId, SessionEnd, commands::PushLogLinesCommand, ports::LogService},
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

/// Why a data plane is stopping.
///
/// Two variants rather than the domain's three: `silent` is the control
/// plane's own verdict about a data plane that said nothing, so a data plane
/// must not be able to claim it.
#[derive(Deserialize, Serialize, ToSchema, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Ending {
    /// The pods ran out, or the session reached its ceiling.
    Finished,
    /// The pods could not be read at all.
    Unreadable,
}

impl From<Ending> for SessionEnd {
    fn from(ending: Ending) -> Self {
        match ending {
            Ending::Finished => Self::Finished,
            Ending::Unreadable => Self::Unreadable,
        }
    }
}

#[derive(Deserialize, ToSchema)]
pub struct PushLogsRequest {
    pub lines: Vec<LogLine>,
    /// Set when nothing further will be sent for this session, and why.
    ///
    /// A request with no lines and no ending is a data plane saying it is
    /// still following a quiet instance, which is what keeps the read open.
    #[serde(default)]
    pub ending: Option<Ending>,
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
    description = "Lines are relayed to whoever opened the session and are never written down. A request with no lines and no ending says the data plane is still following an instance that has nothing to report, which is what keeps a quiet read from being taken for a dead one. A session nobody is reading any more is accepted and discarded, because the data plane had no way to know the reader left; `listening` says so, and is the signal to stop sending.",
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
                ending: request.ending.map(SessionEnd::from),
            },
        )
        .await?;

    Ok(Response::OK(PushLogsResponse {
        data: PushLogsResponseData { relayed, listening },
    }))
}
