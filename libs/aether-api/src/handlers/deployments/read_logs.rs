use std::{convert::Infallible, time::Duration};

use aether_auth::Identity;
use aether_core::{
    deployments::DeploymentId,
    logs::{
        LogLine, LogSessionId, LogWindow, Relayed, SessionEnd,
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
use serde::{Deserialize, Serialize};
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

/// How long a session may stay open with nothing at all from the data plane.
///
/// Measured against the data plane, not against the instance. A data plane
/// following something quiet says so on its own schedule, so this only fires
/// when nobody is on the other end -- which is the one case where holding the
/// connection open helps nobody.
const SILENCE: Duration = Duration::from_secs(30);

/// One line, on its way to the reader.
fn line_event(line: LogLine) -> Event {
    Event::default()
        .event("line")
        .json_data(&line)
        .unwrap_or_else(|_| Event::default().event("line").data(line.message))
}

/// The last frame of a session, saying why there will be no more.
///
/// Sent before the connection closes. A stream that simply stops is
/// indistinguishable from a network that dropped, and a reader who cannot
/// tell those apart is left watching a screen that will never change.
fn ended_event(end: SessionEnd) -> Event {
    #[derive(Serialize)]
    struct Ended {
        reason: SessionEnd,
    }

    Event::default()
        .event("ended")
        .json_data(Ended { reason: end })
        .unwrap_or_else(|_| Event::default().event("ended").data("finished"))
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/deployments/{deployment_id}/logs",
    summary = "read an instance's logs as they arrive",
    tag = "deployments",
    description = "Server sent events. Lines are relayed from the data plane and never stored: there is no endpoint to read them back afterwards. Every read is recorded in the audit log, and the window is capped by the server.",
    params(ReadLogsRoute, ReadLogsQuery),
    responses(
        (status = 200, description = "A stream of log lines, ended by an `ended` event saying why"),
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

    // `None` as the state is a session that has already said why it ended:
    // the frame went out, and there is nothing after it.
    let events = stream::unfold(Some(stream), |state| async move {
        let mut stream = state?;

        loop {
            let relayed = match tokio::time::timeout(SILENCE, stream.next()).await {
                // Not one word from the data plane for long enough to call it
                // gone. Every other case below has an answer from it.
                Err(_) => {
                    return Some((
                        Ok::<Event, Infallible>(ended_event(SessionEnd::Silent)),
                        None,
                    ));
                }
                // The relay dropped the session without a reason. It sends one
                // first, so this is the path that should not happen; saying
                // something is still better than a connection that goes quiet.
                Ok(None) => {
                    return Some((
                        Ok::<Event, Infallible>(ended_event(SessionEnd::Finished)),
                        None,
                    ));
                }
                Ok(Some(relayed)) => relayed,
            };

            match relayed {
                Relayed::Line(line) => {
                    return Some((Ok::<Event, Infallible>(line_event(line)), Some(stream)));
                }
                // Never shown. Its whole job is to have reset the timeout
                // above, which is what stops a quiet instance from being
                // taken for a dead data plane.
                Relayed::StillFollowing => continue,
                Relayed::Ended(end) => {
                    return Some((Ok::<Event, Infallible>(ended_event(end)), None));
                }
            }
        }
    });

    Ok(Sse::new(events).keep_alive(KeepAlive::default()))
}
