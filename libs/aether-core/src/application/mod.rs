use std::sync::Arc;

use aether_auth::KeycloakAuthRepository;
use sqlx::PgPool;

use crate::{
    AetherConfig, CoreError, auth::service::AuthServiceImpl,
    infrastructure::organisation::PostgresOrganisationRepository,
    organisation::service::OrganisationServiceImpl,
};

mod auth;
mod organisation;

type OrganisationRepo = PostgresOrganisationRepository;
type AuthRepo = KeycloakAuthRepository;

#[derive(Clone)]
pub struct AetherService {
    pub(crate) organisation_service: OrganisationServiceImpl<OrganisationRepo>,
    pub(crate) auth_service: AuthServiceImpl<AuthRepo>,
}

pub async fn create_service(config: AetherConfig) -> Result<AetherService, CoreError> {
    let database_url = format!(
        "postgres://{}:{}@{}:{}/{}",
        config.database.username,
        config.database.password,
        config.database.host,
        config.database.port,
        config.database.name
    );

    let pg_pool = PgPool::connect(&database_url)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: e.to_string(),
        })?;

    let organisation_repository = Arc::new(PostgresOrganisationRepository::new(pg_pool));
    let auth_repository = Arc::new(KeycloakAuthRepository::new("issuer", None));

    Ok(AetherService {
        organisation_service: OrganisationServiceImpl::new(organisation_repository.clone()),
        auth_service: AuthServiceImpl::new(auth_repository.clone()),
    })
}
