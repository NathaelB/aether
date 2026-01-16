use std::{future::Future, sync::Arc};

use tokio::task_local;

use crate::domain::{
    auth::ports::MockAuthService,
    action::ports::MockActionRepository,
    deployments::ports::MockDeploymentRepository,
    organisation::ports::MockOrganisationRepository,
    role::ports::MockRoleRepository,
};

task_local! {
    static AUTH_SERVICE: Arc<MockAuthService>;
}

task_local! {
    static ROLE_REPOSITORY: Arc<MockRoleRepository>;
}

task_local! {
    static ORGANISATION_REPOSITORY: Arc<MockOrganisationRepository>;
}

task_local! {
    static DEPLOYMENT_REPOSITORY: Arc<MockDeploymentRepository>;
}

task_local! {
    static ACTION_REPOSITORY: Arc<MockActionRepository>;
}

pub async fn with_auth_service<F, Fut, T>(auth: MockAuthService, f: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    AUTH_SERVICE.scope(Arc::new(auth), f()).await
}

pub async fn scope_auth_service<T, Fut>(auth: Arc<MockAuthService>, fut: Fut) -> T
where
    Fut: Future<Output = T>,
{
    AUTH_SERVICE.scope(auth, fut).await
}

pub async fn with_role_repository<F, Fut, T>(repo: MockRoleRepository, f: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    ROLE_REPOSITORY.scope(Arc::new(repo), f()).await
}

pub async fn scope_role_repository<T, Fut>(repo: Arc<MockRoleRepository>, fut: Fut) -> T
where
    Fut: Future<Output = T>,
{
    ROLE_REPOSITORY.scope(repo, fut).await
}

pub async fn with_organisation_repository<F, Fut, T>(repo: MockOrganisationRepository, f: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    ORGANISATION_REPOSITORY.scope(Arc::new(repo), f()).await
}

pub async fn with_deployment_repository<F, Fut, T>(repo: MockDeploymentRepository, f: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    DEPLOYMENT_REPOSITORY.scope(Arc::new(repo), f()).await
}

pub async fn with_action_repository<F, Fut, T>(repo: MockActionRepository, f: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    ACTION_REPOSITORY.scope(Arc::new(repo), f()).await
}

pub fn auth_service() -> Option<Arc<MockAuthService>> {
    AUTH_SERVICE.try_with(|auth| auth.clone()).ok()
}

pub fn role_repository() -> Option<Arc<MockRoleRepository>> {
    ROLE_REPOSITORY.try_with(|repo| repo.clone()).ok()
}

pub fn organisation_repository() -> Option<Arc<MockOrganisationRepository>> {
    ORGANISATION_REPOSITORY.try_with(|repo| repo.clone()).ok()
}

pub fn deployment_repository() -> Option<Arc<MockDeploymentRepository>> {
    DEPLOYMENT_REPOSITORY.try_with(|repo| repo.clone()).ok()
}

pub fn action_repository() -> Option<Arc<MockActionRepository>> {
    ACTION_REPOSITORY.try_with(|repo| repo.clone()).ok()
}
