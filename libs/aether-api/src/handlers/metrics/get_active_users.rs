use aether_auth::Identity;
use aether_core::{
    deployments::DeploymentId, metrics::commands::ActiveUsersQuery, metrics::ports::MetricsService,
    organisation::OrganisationId,
};
use axum::{
    Extension,
    extract::{Query, State},
};
use axum_extra::routing::TypedPath;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{errors::ApiError, response::Response, state::AppState};

#[derive(TypedPath, IntoParams, Deserialize)]
#[typed_path("/organisations/{organisation_id}/deployments/{deployment_id}/active-users")]
pub struct GetActiveUsersRoute {
    pub organisation_id: Uuid,
    pub deployment_id: Uuid,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct GetActiveUsersQuery {
    /// The counting window, in minutes. Never defaulted to a constant here:
    /// whoever asks states what they mean by "active", and the platform's one
    /// definition answers the same way for whichever window they choose.
    pub window_minutes: i64,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct GetActiveUsersResponseData {
    /// `null` when the deployment has not reported inside the window -- a
    /// different fact from zero, which the console must not collapse into it.
    pub active_users: Option<u64>,
}

#[derive(Serialize, ToSchema, PartialEq)]
pub struct GetActiveUsersResponse {
    data: GetActiveUsersResponseData,
}

#[utoipa::path(
    get,
    path = "/{organisation_id}/deployments/{deployment_id}/active-users",
    summary = "count active users for a deployment over a window",
    tag = "metrics",
    description = "The number of active users a deployment reported within the last \
                   `window_minutes`, using the one definition of an active user every \
                   caller shares. Null when the deployment has not reported inside the \
                   window, which is distinct from zero. Requires VIEW_INSTANCES.",
    params(GetActiveUsersRoute, GetActiveUsersQuery),
    responses(
        (status = 200, description = "The active user count", body = GetActiveUsersResponse),
        (status = 401, description = "Unauthorized", body = ApiError),
        (status = 403, description = "Forbidden", body = ApiError),
        (status = 500, description = "Internal Server Error", body = ApiError)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_active_users_handler(
    GetActiveUsersRoute {
        organisation_id,
        deployment_id,
    }: GetActiveUsersRoute,
    Query(query): Query<GetActiveUsersQuery>,
    State(state): State<AppState>,
    Extension(identity): Extension<Identity>,
) -> Result<Response<GetActiveUsersResponse>, ApiError> {
    // `Utc::now()` belongs here and nowhere further in: this is the edge, and
    // the counting rule itself always takes `at` as a parameter.
    let active_users_query = ActiveUsersQuery::new(
        OrganisationId(organisation_id),
        DeploymentId(deployment_id),
        Utc::now(),
        Duration::minutes(query.window_minutes),
    );

    let active_users = state
        .service
        .active_users_for_deployment(identity, active_users_query)
        .await?;

    Ok(Response::OK(GetActiveUsersResponse {
        data: GetActiveUsersResponseData { active_users },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{app_state, user_identity};

    #[tokio::test]
    async fn a_service_error_reaches_the_caller_as_an_api_error() {
        let state = app_state();

        let result = get_active_users_handler(
            GetActiveUsersRoute {
                organisation_id: Uuid::new_v4(),
                deployment_id: Uuid::new_v4(),
            },
            Query(GetActiveUsersQuery { window_minutes: 30 }),
            State(state),
            Extension(user_identity("user-1")),
        )
        .await;

        assert!(result.is_err());
    }
}
