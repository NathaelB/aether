use axum::{Router, middleware::from_fn_with_state};
use axum_extra::routing::RouterExt;
use utoipa::OpenApi;

use crate::{
    handlers::deployments::{
        create_deployment::{__path_create_deployment_handler, create_deployment_handler},
        delete_deployment::{__path_delete_deployment_handler, delete_deployment_handler},
        get_deployment::{__path_get_deployment_handler, get_deployment_handler},
        list_deployments::{__path_list_deployments_handler, list_deployments_handler},
        update_deployment::{__path_update_deployment_handler, update_deployment_handler},
    },
    router::service_auth_middleware,
    state::AppState,
};

pub mod create_deployment;
pub mod delete_deployment;
pub mod get_deployment;
pub mod list_deployments;
pub mod update_deployment;

#[derive(OpenApi)]
#[openapi(
    paths(
        list_deployments_handler,
        create_deployment_handler,
        get_deployment_handler,
        update_deployment_handler,
        delete_deployment_handler,
    ),
    tags(
        (name = "deployments", description = "Deployment management endpoints scoped to organisations.")
    )
)]
pub struct DeploymentApiDoc;

pub fn deployment_routes(app_state: AppState) -> Router<AppState> {
    Router::new()
        .typed_get(list_deployments_handler)
        .typed_post(create_deployment_handler)
        .typed_get(get_deployment_handler)
        .typed_patch(update_deployment_handler)
        .typed_delete(delete_deployment_handler)
        .layer(from_fn_with_state(
            app_state.clone(),
            service_auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::deployment_routes;
    use crate::test_helpers::app_state;

    #[tokio::test]
    async fn deployment_routes_builds() {
        let state = app_state();
        let _router = deployment_routes(state);
    }
}
