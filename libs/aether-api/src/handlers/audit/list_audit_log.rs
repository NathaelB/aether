use aether_auth::Identity;
use aether_core::{
    audit::{AuditCursor, AuditEntry, commands::ListAuditEntriesCommand, ports::AuditService},
    organisation::OrganisationId,
};
use axum::{
    Extension,
    extract::{Query, State},
};
use axum_extra::routing::TypedPath;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, handlers::default_limit, response::Response, state::AppState};

#[derive(Serialize, ToSchema, PartialEq)]
pub struct ListAuditLogResponse {
    data: Vec<AuditEntry>,
    next_cursor: Option<String>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListAuditLogQuery {
    cursor: Option<String>,

    /// A caller exporting the trail rather than paging through a screen
    /// passes a value up to `ListAuditEntriesCommand::MAX_LIMIT` here; there
    /// is no separate export endpoint.
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/audit-log")]
pub struct ListAuditLogRoute {
    pub organisation_id: Uuid,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/audit-log",
    summary = "list audit log entries",
    tag = "audit",
    description = "List an organisation's audit trail, newest first. Paginated with a keyset cursor so a page stays stable while new entries are recorded. Requires VIEW_ORGANISATION.",
    params(ListAuditLogRoute, ListAuditLogQuery),
    responses(
        (status = 200, description = "Audit log entries, newest first", body = ListAuditLogResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Forbidden", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_audit_log_handler(
    ListAuditLogRoute { organisation_id }: ListAuditLogRoute,
    Query(query): Query<ListAuditLogQuery>,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<ListAuditLogResponse>, ApiError> {
    let organisation_id = OrganisationId(organisation_id);

    let mut command = ListAuditEntriesCommand::new(organisation_id, query.limit);
    if let Some(cursor) = query.cursor {
        command = command.with_cursor(AuditCursor::new(cursor));
    }

    let batch = state.service.list_entries(identity, command).await?;

    Ok(Response::OK(ListAuditLogResponse {
        data: batch.entries,
        next_cursor: batch.next_cursor.map(|cursor| cursor.0),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};

    #[tokio::test]
    async fn list_audit_log_maps_service_error() {
        let state = app_state();
        let identity = user_identity("user-123");

        let result = list_audit_log_handler(
            ListAuditLogRoute {
                organisation_id: Uuid::new_v4(),
            },
            Query(ListAuditLogQuery {
                cursor: None,
                limit: 10,
            }),
            State(state),
            Extension(identity),
        )
        .await;

        assert!(matches!(result, Err(ApiError::Unknown { .. })));
    }
}
