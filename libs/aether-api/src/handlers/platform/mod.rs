use axum::{Router, middleware::from_fn_with_state};
use axum_extra::routing::RouterExt;
use utoipa::OpenApi;

use crate::{
    handlers::platform::{
        fleet_audit_log::{__path_list_fleet_audit_log_handler, list_fleet_audit_log_handler},
        list_estate_deployments::{
            __path_list_estate_deployments_handler, list_estate_deployments_handler,
        },
        list_tenants::{
            __path_get_tenant_handler, __path_list_tenants_handler, get_tenant_handler,
            list_tenants_handler,
        },
        move_tenant_plan::{__path_move_tenant_plan_handler, move_tenant_plan_handler},
        my_rights::{__path_my_rights_handler, my_rights_handler},
        operators::{
            __path_grant_operator_handler, __path_list_operators_handler,
            __path_revoke_operator_handler, grant_operator_handler, list_operators_handler,
            revoke_operator_handler,
        },
    },
    router::service_auth_middleware,
    state::AppState,
};

pub mod fleet_audit_log;
pub mod list_estate_deployments;
pub mod list_tenants;
pub mod move_tenant_plan;
pub mod my_rights;
pub mod operators;

#[derive(OpenApi)]
#[openapi(
    paths(
        list_estate_deployments_handler,
        list_fleet_audit_log_handler,
        list_tenants_handler,
        get_tenant_handler,
        move_tenant_plan_handler,
        my_rights_handler,
        list_operators_handler,
        grant_operator_handler,
        revoke_operator_handler
    ),
    tags(
        (name = "platform", description = "What the installation runs, for whoever runs it.")
    )
)]
pub struct PlatformApiDoc;

pub fn platform_routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .typed_get(list_estate_deployments_handler)
        .typed_get(list_fleet_audit_log_handler)
        .typed_get(list_tenants_handler)
        .typed_get(get_tenant_handler)
        .typed_put(move_tenant_plan_handler)
        .typed_get(my_rights_handler)
        .typed_get(list_operators_handler)
        .typed_put(grant_operator_handler)
        .typed_delete(revoke_operator_handler)
        .layer(from_fn_with_state(
            app_state.clone(),
            service_auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::platform_routes;
    use crate::test_helpers::app_state;

    #[tokio::test]
    async fn platform_routes_builds() {
        let _router = platform_routes(app_state());
    }
}
