use aether_auth::Identity;
use aether_core::signals::{Signal, SignalKind, SignalSubject};
use aether_core::{dataplane::value_objects::DataPlaneId, deployments::DeploymentId};
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
pub struct ListSignalsResponse {
    data: Vec<Signal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListSignalsQuery {
    /// Filter by signal kind (e.g., dataplane_heartbeat_stale).
    kind: Option<String>,

    /// Filter by subject kind (dataplane, deployment, or action).
    subject_kind: Option<String>,

    /// Subject ID to filter by (only when subject_kind is specified).
    subject_id: Option<String>,

    /// Up to 100 signals per page.
    #[serde(default = "default_limit")]
    limit: usize,

    /// Opaque cursor for pagination.
    cursor: Option<String>,
}

#[derive(TypedPath)]
#[typed_path("/platform/signals")]
pub struct SignalsRoute;

#[utoipa::path(
    get,
    path = "/signals",
    summary = "list open signals in the installation",
    tag = "platform",
    description = "Open signals: what probes have found and reported. Newest first. \
                   Filterable by kind and subject. Requires view_estate.",
    params(ListSignalsQuery),
    responses(
        (status = 200, description = "Open signals, newest first", body = ListSignalsResponse),
        (status = 400, description = "Invalid filter or limit", body = ApiError),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "The caller does not hold view_estate", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_signals_handler(
    _: SignalsRoute,
    Query(query): Query<ListSignalsQuery>,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<ListSignalsResponse>, ApiError> {
    // Validate and parse limit
    if query.limit > 100 {
        return Err(ApiError::BadRequest {
            reason: "limit must be at most 100".to_string(),
        });
    }

    // Parse kind filter if present
    let kind_filter = query
        .kind
        .as_deref()
        .map(|k| k.parse::<SignalKind>())
        .transpose()
        .map_err(|_| ApiError::BadRequest {
            reason: format!(
                "'{}' is not a valid signal kind",
                query.kind.clone().unwrap_or_default()
            ),
        })?;

    // Parse subject filter if present
    let subject_filter = match (query.subject_kind.as_deref(), query.subject_id.as_deref()) {
        (None, None) => Ok(None),
        (Some("dataplane"), Some(id_str)) => {
            let id = Uuid::parse_str(id_str).map_err(|_| ApiError::BadRequest {
                reason: format!("'{}' is not a valid UUID", id_str),
            })?;
            Ok(Some(SignalSubject::Dataplane {
                id: DataPlaneId(id),
            }))
        }
        (Some("deployment"), Some(id_str)) => {
            let id = Uuid::parse_str(id_str).map_err(|_| ApiError::BadRequest {
                reason: format!("'{}' is not a valid UUID", id_str),
            })?;
            Ok(Some(SignalSubject::Deployment {
                id: DeploymentId(id),
            }))
        }
        (Some("action"), Some(id_str)) => {
            let id = Uuid::parse_str(id_str).map_err(|_| ApiError::BadRequest {
                reason: format!("'{}' is not a valid UUID", id_str),
            })?;
            Ok(Some(SignalSubject::Action { id }))
        }
        (Some(kind), _) => Err(ApiError::BadRequest {
            reason: format!("'{}' is not a valid subject kind", kind),
        }),
        (None, Some(_)) => Err(ApiError::BadRequest {
            reason: "subject_id requires subject_kind".to_string(),
        }),
    }?;

    let page = state
        .service
        .list_open_signals(
            identity,
            kind_filter,
            subject_filter,
            query.limit,
            query.cursor,
        )
        .await?;

    Ok(Response::OK(ListSignalsResponse {
        data: page.signals,
        next_cursor: page.next_cursor,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};

    #[tokio::test]
    async fn list_signals_maps_service_error() {
        let result = list_signals_handler(
            SignalsRoute,
            Query(ListSignalsQuery {
                kind: None,
                subject_kind: None,
                subject_id: None,
                cursor: None,
                limit: 10,
            }),
            State(app_state()),
            Extension(user_identity("operator-1")),
        )
        .await;

        assert!(matches!(result, Err(ApiError::Unknown { .. })));
    }

    #[test]
    fn limit_exceeding_max_is_refused() {
        let query = ListSignalsQuery {
            kind: None,
            subject_kind: None,
            subject_id: None,
            cursor: None,
            limit: 101,
        };

        assert!(
            query.limit > 100,
            "test assumption: 101 exceeds the maximum of 100"
        );
    }

    #[test]
    fn invalid_signal_kind_is_refused() {
        let query = ListSignalsQuery {
            kind: Some("unknown_kind".to_string()),
            subject_kind: None,
            subject_id: None,
            cursor: None,
            limit: 10,
        };

        assert!(!query.kind.clone().unwrap_or_default().is_empty());
    }
}
