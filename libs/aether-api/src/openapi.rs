use utoipa::OpenApi;

use crate::handlers::{
    actions::ActionApiDoc, dataplanes::DataPlaneApiDoc, deployments::DeploymentApiDoc,
    organisations::OrganisationApiDoc, roles::RoleApiDoc, users::UserApiDoc,
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Aether API",
        version = "0.1.0",
        description = "API documentation for Aether services"
    ),
    nest(
        (path = "/organisations", api = OrganisationApiDoc),
        (path = "/organisations", api = RoleApiDoc),
        (path = "/organisations", api = DeploymentApiDoc),
        (path = "/organisations", api = ActionApiDoc),
        (path = "/users", api = UserApiDoc),
        (path = "/dataplanes", api = DataPlaneApiDoc),
    )
)]
pub struct ApiDoc;

#[cfg(test)]
mod tests {
    use super::ApiDoc;
    use utoipa::OpenApi;

    #[test]
    fn openapi_has_title() {
        let doc = ApiDoc::openapi();
        assert_eq!(doc.info.title, "Aether API");
    }
}
