use aether_auth::Identity;
use aether_core::{
    organisation::OrganisationId,
    traces::{TraceDetail, commands::ReadTraceCommand},
};
use axum::{Extension, extract::State};
use axum_extra::routing::TypedPath;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/traces/{trace_id}")]
pub struct GetTraceRoute {
    pub organisation_id: Uuid,
    pub trace_id: String,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/traces/{trace_id}",
    summary = "every span of one trace",
    tag = "traces",
    description = "Every span sharing one trace id, ordered by start time -- what a waterfall \
                   view is built from. Not scoped by a search's own window or filters: a reader \
                   already holds a trace id because a search hit named it, and this reads that \
                   trace whole. Every read is recorded in the audit log, the same as a search. \
                   Requires READ_INSTANCE_LOGS.",
    params(GetTraceRoute),
    responses(
        (status = 200, description = "Every span of this trace, oldest first", body = TraceDetail),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not read these traces", body = ApiError),
        (status = 409, description = "This installation has no Quickwit configured", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_trace_handler(
    GetTraceRoute {
        organisation_id,
        trace_id,
    }: GetTraceRoute,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<TraceDetail>, ApiError> {
    let search_index = state
        .quickwit_traces
        .clone()
        .ok_or_else(|| ApiError::Conflict {
            reason: "trace search is not configured on this installation".to_string(),
        })?;

    let command = ReadTraceCommand {
        organisation_id: OrganisationId(organisation_id),
        trace_id,
    };

    let result = state
        .service
        .get_trace(identity, command, search_index.as_ref())
        .await?;

    Ok(Response::OK(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};

    #[tokio::test]
    async fn without_quickwit_configured_the_read_is_a_conflict() {
        let state = app_state();
        assert!(state.quickwit_traces.is_none());

        let result = get_trace_handler(
            GetTraceRoute {
                organisation_id: Uuid::new_v4(),
                trace_id: "abc123".to_string(),
            },
            State(state),
            Extension(user_identity("user-1")),
        )
        .await;

        assert!(matches!(result, Err(ApiError::Conflict { .. })));
    }
}
