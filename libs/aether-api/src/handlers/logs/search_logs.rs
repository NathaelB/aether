use aether_auth::Identity;
use aether_core::{
    deployments::DeploymentId,
    logs::{LogLevel, LogSearchResult, LogSearchWindow, commands::SearchLogsCommand},
    organisation::OrganisationId,
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
#[typed_path("/organisations/{organisation_id}/logs/search")]
pub struct SearchLogsRoute {
    pub organisation_id: Uuid,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchLogsQuery {
    /// Start of the range, inclusive.
    pub from: DateTime<Utc>,
    /// End of the range, exclusive. Absolute, like `from` -- an offset from
    /// "now" would make the same request cover a different span each time it
    /// is sent.
    pub to: DateTime<Utc>,
    /// The lowest severity to include: one of `trace`, `debug`, `info`,
    /// `warn`, `error`, `fatal`. `unknown` -- a line whose format Herald
    /// could not read -- is always included, whatever this is set to, and is
    /// not itself a valid floor.
    pub level_floor: String,
    /// Free text, matched against the line's own message.
    pub q: Option<String>,
    /// Narrows to one deployment. Left out, every deployment in the
    /// organisation's index is searched.
    pub deployment_id: Option<Uuid>,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/logs/search",
    summary = "search an organisation's indexed logs",
    tag = "logs",
    description = "Translates a narrow, typed query -- a time range, a level floor, free text \
                   and an optional deployment -- into a query run against this organisation's \
                   own Quickwit index. Not a pass-through of Quickwit's own query language: a \
                   caller cannot send one. Every search is recorded in the audit log, the same \
                   as the live tail. Requires READ_INSTANCE_LOGS.",
    params(SearchLogsRoute, SearchLogsQuery),
    responses(
        (status = 200, description = "Hits within the requested window and floor, newest first", body = LogSearchResult),
        (status = 400, description = "The window is invalid, or spans more than the index retains", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not read these logs", body = ApiError),
        (status = 404, description = "The deployment filter names one that is not this organisation's", body = ApiError),
        (status = 409, description = "This installation has no Quickwit configured", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn search_logs_handler(
    SearchLogsRoute { organisation_id }: SearchLogsRoute,
    Query(query): Query<SearchLogsQuery>,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<LogSearchResult>, ApiError> {
    let window = LogSearchWindow::new(query.from, query.to)?;
    let level_floor = query
        .level_floor
        .parse::<LogLevel>()
        .map_err(|reason| ApiError::BadRequest { reason })?;

    let command = SearchLogsCommand {
        organisation_id: OrganisationId(organisation_id),
        deployment_id: query.deployment_id.map(DeploymentId),
        window,
        level_floor,
        text: query.q.filter(|text| !text.trim().is_empty()),
    };

    // Not configured is a first-class state, the same way an installation
    // with no certificate source distributes none rather than the request
    // failing to connect somewhere: told apart from every other refusal so
    // the caller does not read it as their own mistake.
    let search_index = state
        .quickwit_search
        .clone()
        .ok_or_else(|| ApiError::Conflict {
            reason: "log search is not configured on this installation".to_string(),
        })?;

    let result = state
        .service
        .search_logs(identity, command, search_index.as_ref())
        .await?;

    Ok(Response::OK(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};

    fn query() -> SearchLogsQuery {
        SearchLogsQuery {
            from: DateTime::parse_from_rfc3339("2026-09-01T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            to: DateTime::parse_from_rfc3339("2026-09-02T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            level_floor: "warn".to_string(),
            q: None,
            deployment_id: None,
        }
    }

    /// No Quickwit configured is refused before the service is even asked,
    /// which also means it never touches the database this test's `app_state`
    /// cannot reach.
    #[tokio::test]
    async fn without_quickwit_configured_the_search_is_a_conflict() {
        let state = app_state();
        assert!(state.quickwit_search.is_none());

        let result = search_logs_handler(
            SearchLogsRoute {
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
    async fn an_unknown_level_name_is_a_bad_request() {
        let state = app_state();
        let mut invalid = query();
        invalid.level_floor = "critical".to_string();

        let result = search_logs_handler(
            SearchLogsRoute {
                organisation_id: Uuid::new_v4(),
            },
            Query(invalid),
            State(state),
            Extension(user_identity("user-1")),
        )
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest { .. })));
    }

    #[tokio::test]
    async fn an_invalid_window_is_a_bad_request_before_anything_else_runs() {
        let state = app_state();
        let mut invalid = query();
        invalid.to = invalid.from;

        let result = search_logs_handler(
            SearchLogsRoute {
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
