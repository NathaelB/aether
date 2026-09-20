use aether_auth::Identity;
use aether_core::audit::{
    AuditCursor,
    fleet::{FleetAuditEntry, commands::ListFleetAuditEntriesCommand, ports::FleetAuditService},
};
use axum::{
    Extension,
    extract::{Query, State},
};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{errors::ApiError, handlers::default_limit, response::Response, state::AppState};

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ListFleetAuditLogResponse {
    data: Vec<FleetAuditEntry>,
    next_cursor: Option<String>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListFleetAuditLogQuery {
    cursor: Option<String>,

    /// Up to `ListFleetAuditEntriesCommand::MAX_LIMIT`, matching the
    /// organisation trail: a caller exporting the whole thing does it in a
    /// handful of requests rather than through a second endpoint.
    #[serde(default = "default_limit")]
    limit: usize,
}

/// Under `/platform`, not under an organisation, and there is no organisation
/// in the path to make it otherwise. That is the routing half of what keeps a
/// customer out of this trail; the other half is that nothing below takes a
/// scope to filter on.
#[derive(TypedPath)]
#[typed_path("/platform/audit-log")]
pub struct FleetAuditLogRoute;

#[utoipa::path(
    get,
    path = "/audit-log",
    summary = "list what was done to the installation",
    tag = "platform",
    description = "The fleet's own trail, newest first: a data plane registered, drained, \
                   disabled or returned to service, a credential re-issued, an operator \
                   granted or revoked. Separate from an organisation's trail, which it never \
                   contains and is never contained by. Paginated with a keyset cursor so a \
                   page stays stable while new entries are recorded. Requires view_estate.",
    params(ListFleetAuditLogQuery),
    responses(
        (status = 200, description = "Fleet audit entries, newest first", body = ListFleetAuditLogResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller does not hold view_estate", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_fleet_audit_log_handler(
    _: FleetAuditLogRoute,
    Query(query): Query<ListFleetAuditLogQuery>,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<ListFleetAuditLogResponse>, ApiError> {
    let mut command = ListFleetAuditEntriesCommand::new(query.limit);
    if let Some(cursor) = query.cursor {
        command = command.with_cursor(AuditCursor::new(cursor));
    }

    let batch = state.service.list_fleet_entries(identity, command).await?;

    Ok(Response::OK(ListFleetAuditLogResponse {
        data: batch.entries,
        next_cursor: batch.next_cursor.map(|cursor| cursor.0),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};

    #[tokio::test]
    async fn list_fleet_audit_log_maps_service_error() {
        let result = list_fleet_audit_log_handler(
            FleetAuditLogRoute,
            Query(ListFleetAuditLogQuery {
                cursor: None,
                limit: 10,
            }),
            State(app_state()),
            Extension(user_identity("operator-1")),
        )
        .await;

        assert!(matches!(result, Err(ApiError::Unknown { .. })));
    }
}
