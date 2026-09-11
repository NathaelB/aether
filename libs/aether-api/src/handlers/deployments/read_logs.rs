use std::{convert::Infallible, time::Duration};

use aether_auth::Identity;
use aether_core::{
    deployments::DeploymentId,
    logs::{
        LogSessionId, LogWindow,
        commands::ReadLogsCommand,
        ports::{LogService, LogStream},
    },
    organisation::OrganisationId,
};
use axum::{
    Extension,
    extract::State,
    response::{
        IntoResponse, Sse,
        sse::{Event, KeepAlive},
    },
};
use axum_extra::routing::TypedPath;
use futures::stream;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::{errors::ApiError, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments/{deployment_id}/logs")]
pub struct ReadLogsRoute {
    pub organisation_id: Uuid,
    pub deployment_id: Uuid,
}

#[derive(Deserialize, IntoParams)]
pub struct ReadLogsQuery {
    /// How far back to reach. Capped by the server, not by the screen.
    pub since_minutes: i64,
}

/// How long a session may stay open with nothing arriving.
///
/// A data plane that never answers, or one that finished without saying so,
/// must not hold a connection open for ever.
const SILENCE: Duration = Duration::from_secs(30);

#[utoipa::path(
    get,
    path = "/{organisation_id}/deployments/{deployment_id}/logs",
    summary = "read an instance's logs as they arrive",
    tag = "deployments",
    description = "Server sent events. Lines are relayed from the data plane and never stored: there is no endpoint to read them back afterwards. Every read is recorded in the audit log, and the window is capped by the server.",
    params(ReadLogsRoute, ReadLogsQuery),
    responses(
        (status = 200, description = "A stream of log lines"),
        (status = 400, description = "The window asked for is longer than the cap", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not read these logs", body = ApiError),
        (status = 404, description = "No such deployment", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn read_logs_handler(
    ReadLogsRoute {
        organisation_id,
        deployment_id,
    }: ReadLogsRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
    axum::extract::Query(query): axum::extract::Query<ReadLogsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let window = LogWindow::minutes(query.since_minutes)?;

    let (_session, stream) = state
        .service
        .read_logs(
            identity,
            ReadLogsCommand {
                organisation_id: OrganisationId(organisation_id),
                deployment_id: DeploymentId(deployment_id),
                window,
                session_id: LogSessionId(Uuid::new_v4()),
            },
        )
        .await?;

    let events = stream::unfold(stream, |mut stream| async move {
        // A line that never comes ends the stream rather than holding the
        // connection open on a data plane that has gone quiet.
        let line = tokio::time::timeout(SILENCE, stream.next_line())
            .await
            .ok()??;

        let event = Event::default()
            .event("line")
            .json_data(&line)
            .unwrap_or_else(|_| Event::default().event("line").data(line.message));

        Some((Ok::<Event, Infallible>(event), stream))
    });

    Ok(Sse::new(events).keep_alive(KeepAlive::default()))
}
