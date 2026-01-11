use std::sync::Arc;

use aether_auth::KeycloakAuthRepository;
use sqlx::PgPool;

use crate::{
    AetherConfig, CoreError,
    auth::service::AuthServiceImpl,
    deployments::service::DeploymentServiceImpl,
    infrastructure::{
        deployments::PostgresDeploymentRepository, organisation::PostgresOrganisationRepository,
        role::PostgresRoleRepository,
    },
    organisation::service::OrganisationServiceImpl,
    role::service::RoleServiceImpl,
};

mod auth;
mod deployment;
mod organisation;
mod role;

type OrganisationRepo = PostgresOrganisationRepository;
type AuthRepo = KeycloakAuthRepository;
type RoleRepo = PostgresRoleRepository;
type DeploymentRepo = PostgresDeploymentRepository;

#[derive(Clone)]
pub struct AetherService {
    pub(crate) organisation_service: OrganisationServiceImpl<OrganisationRepo>,
    pub(crate) auth_service: AuthServiceImpl<AuthRepo>,
    pub(crate) role_service: RoleServiceImpl<RoleRepo>,
    pub(crate) deployment_service: DeploymentServiceImpl<DeploymentRepo>,
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

    let organisation_repository = Arc::new(PostgresOrganisationRepository::new(pg_pool.clone()));
    let auth_repository = Arc::new(KeycloakAuthRepository::new(config.auth.issuer, None));
    let role_repository = Arc::new(PostgresRoleRepository::new(pg_pool.clone()));
    let deployment_repository = Arc::new(PostgresDeploymentRepository::new(pg_pool));

    Ok(AetherService {
        organisation_service: OrganisationServiceImpl::new(organisation_repository.clone()),
        auth_service: AuthServiceImpl::new(auth_repository.clone()),
        role_service: RoleServiceImpl::new(role_repository.clone()),
        deployment_service: DeploymentServiceImpl::new(deployment_repository.clone()),
    })
}
