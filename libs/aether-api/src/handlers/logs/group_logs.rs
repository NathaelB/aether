use aether_auth::Identity;
use aether_core::{
    deployments::DeploymentId,
    logs::{LogGroupResult, LogLevel, LogSearchWindow, commands::SearchLogsCommand},
    organisation::OrganisationId,
};
use axum::{
    Extension,
    extract::{Query, State},
};
use axum_extra::routing::TypedPath;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::{
    errors::ApiError, handlers::logs::search_logs::SearchLogsQuery, response::Response,
    state::AppState,
};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/logs/group")]
pub struct GroupLogsRoute {
    pub organisation_id: Uuid,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/logs/group",
    summary = "group an organisation's indexed logs by fingerprint",
    tag = "logs",
    description = "Takes the same query search does -- a time range, a level floor, free text \
                   and an optional deployment -- and answers with distinct signatures instead of \
                   hits: a burst of the same underlying error is one signature with a count, not \
                   one row per line. A signature is marked `is_new` when it is absent from the \
                   baseline window of equal length immediately preceding the one requested -- \
                   that is 'not seen in the preceding window', not 'never seen before'; a window \
                   reaching past the index's 30-day retention has no baseline data at all, so \
                   everything in it reads as new. Every call is recorded in the audit log, the \
                   same as a search. Requires READ_INSTANCE_LOGS.",
    params(GroupLogsRoute, SearchLogsQuery),
    responses(
        (status = 200, description = "Signatures within the requested window and floor", body = LogGroupResult),
        (status = 400, description = "The window is invalid, or spans more than the index retains", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller may not read these logs", body = ApiError),
        (status = 404, description = "The deployment filter names one that is not this organisation's", body = ApiError),
        (status = 409, description = "This installation has no Quickwit configured", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn group_logs_handler(
    GroupLogsRoute { organisation_id }: GroupLogsRoute,
    Query(query): Query<SearchLogsQuery>,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<LogGroupResult>, ApiError> {
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

    let search_index = state
        .quickwit_search
        .clone()
        .ok_or_else(|| ApiError::Conflict {
            reason: "log search is not configured on this installation".to_string(),
        })?;

    let result = state
        .service
        .group_logs(identity, command, search_index.as_ref())
        .await?;

    Ok(Response::OK(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};
    use chrono::{DateTime, Utc};

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
    /// the same as a plain search.
    #[tokio::test]
    async fn without_quickwit_configured_the_grouping_is_a_conflict() {
        let state = app_state();
        assert!(state.quickwit_search.is_none());

        let result = group_logs_handler(
            GroupLogsRoute {
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

        let result = group_logs_handler(
            GroupLogsRoute {
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

        let result = group_logs_handler(
            GroupLogsRoute {
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
