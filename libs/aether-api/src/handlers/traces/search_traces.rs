use aether_auth::Identity;
use aether_core::{
    deployments::DeploymentId,
    logs::LogSearchWindow,
    organisation::OrganisationId,
    traces::{TraceSearchResult, commands::SearchTracesCommand},
};
use axum::{
    Extension,
    extract::{Query, State},
};
use axum_extra::routing::TypedPath;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/traces/search")]
pub struct SearchTracesRoute {
    pub organisation_id: Uuid,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchTracesQuery {
    /// Start of the range, inclusive.
    pub from: DateTime<Utc>,
    /// End of the range, exclusive -- absolute, like `from`, the same
    /// reasoning as `logs::search_logs::SearchLogsQuery`.
    pub to: DateTime<Utc>,
    /// Narrows to spans naming this `service.name`.
    pub service_name: Option<String>,
    /// Narrows to spans with this OTel status code (`unset`, `ok`, `error`).
    pub status_code: Option<String>,
    /// Free text, matched against a span's own name.
    pub q: Option<String>,
    /// Narrows to one deployment. Left out, every deployment in the
    /// organisation's trace index is searched.
    pub deployment_id: Option<Uuid>,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/traces/search",
    summary = "search an organisation's indexed traces",
    tag = "traces",
    description = "Translates a narrow, typed query -- a time range, an optional service name, \
                   status code and free text, and an optional deployment -- into a query run \
                   against this organisation's own Quickwit trace index, the counterpart to \
                   `logs/search` for OpenTelemetry spans Herald's OTLP receiver shipped. Every \
                   search is recorded in the audit log, the same as a log search. Requires \
                   READ_INSTANCE_LOGS.",
    params(SearchTracesRoute, SearchTracesQuery),
    responses(
        (status = 200, description = "Spans within the requested window, newest first", body = TraceSearchResult),
        (status = 400, description = "The window is invalid, or spans more than the index retains", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not read these traces", body = ApiError),
        (status = 404, description = "The deployment filter names one that is not this organisation's", body = ApiError),
        (status = 409, description = "This installation has no Quickwit configured", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn search_traces_handler(
    SearchTracesRoute { organisation_id }: SearchTracesRoute,
    Query(query): Query<SearchTracesQuery>,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<TraceSearchResult>, ApiError> {
    let window = LogSearchWindow::new(query.from, query.to)?;

    let command = SearchTracesCommand {
        organisation_id: OrganisationId(organisation_id),
        deployment_id: query.deployment_id.map(DeploymentId),
        window,
        service_name: query.service_name.filter(|text| !text.trim().is_empty()),
        status_code: query.status_code.filter(|text| !text.trim().is_empty()),
        text: query.q.filter(|text| !text.trim().is_empty()),
    };

    // Not configured is a first-class state -- the same reasoning
    // `logs::search_logs` gives its own `quickwit_search` check.
    let search_index = state
        .quickwit_traces
        .clone()
        .ok_or_else(|| ApiError::Conflict {
            reason: "trace search is not configured on this installation".to_string(),
        })?;

    let result = state
        .service
        .search_traces(identity, command, search_index.as_ref())
        .await?;

    Ok(Response::OK(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};

    fn query() -> SearchTracesQuery {
        SearchTracesQuery {
            from: DateTime::parse_from_rfc3339("2026-09-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            to: DateTime::parse_from_rfc3339("2026-09-02T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            service_name: None,
            status_code: None,
            q: None,
            deployment_id: None,
        }
    }

    #[tokio::test]
    async fn without_quickwit_configured_the_search_is_a_conflict() {
        let state = app_state();
        assert!(state.quickwit_traces.is_none());

        let result = search_traces_handler(
            SearchTracesRoute {
                organisation_id: Uuid::new_v4(),
            },
            Query(query()),
            State(state),
            Extension(user_identity("user-1")),
        )
        .await;

        assert!(matches!(result, Err(ApiError::Conflict { .. })));
    }

    #[tokio::test]
    async fn an_invalid_window_is_a_bad_request_before_anything_else_runs() {
        let state = app_state();
        let mut invalid = query();
        invalid.to = invalid.from;

        let result = search_traces_handler(
            SearchTracesRoute {
                organisation_id: Uuid::new_v4(),
            },
            Query(invalid),
            State(state),
            Extension(user_identity("user-1")),
        )
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest { .. })));
    }
}
