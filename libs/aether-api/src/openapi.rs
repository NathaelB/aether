use utoipa::OpenApi;

use crate::handlers::organisations::OrganisationApiDoc;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Aether API",
        version = "0.1.0",
        description = "API documentation for Aether services"
    ),
    nest(
        (path = "/organisations", api = OrganisationApiDoc)
    )
)]
pub struct ApiDoc;
