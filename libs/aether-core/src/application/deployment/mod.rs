use crate::{
    AetherService, CoreError,
    action::{
        ActionPayload, ActionSource, ActionTarget, ActionType, ActionVersion, TargetKind,
        commands::RecordActionCommand,
        ports::ActionService,
        service::ActionServiceImpl,
    },
    deployments::{
        Deployment, DeploymentId,
        commands::{CreateDeploymentCommand, UpdateDeploymentCommand},
        ports::DeploymentService,
        service::DeploymentServiceImpl,
    },
    infrastructure::{
        action::PostgresActionRepository,
        deployments::PostgresDeploymentRepository,
        user::PostgresUserRepository,
    },
    organisation::OrganisationId,
};
use serde_json::json;

impl DeploymentService for AetherService {
    async fn create_deployment(
        &self,
        command: CreateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = std::sync::Arc::new(tokio::sync::Mutex::new(Some(tx)));

        run_deployment_transaction(tx, |tx| {
            Box::pin(async move {
                let deployment_repository = PostgresDeploymentRepository::from_tx(tx);
                let user_repository = PostgresUserRepository::from_tx(tx);
                let deployment_service =
                    DeploymentServiceImpl::new(deployment_repository, user_repository);

                let deployment = deployment_service.create_deployment(command).await?;

                let action_repository = PostgresActionRepository::from_tx(tx);
                let action_service = ActionServiceImpl::new(action_repository);
                let action_command = RecordActionCommand::new(
                    ActionType("deployment.create".to_string()),
                    ActionTarget {
                        kind: TargetKind::Deployment,
                        id: deployment.id.0,
                    },
                    ActionPayload {
                        data: json!({
                            "deployment_id": deployment.id.0,
                            "organisation_id": deployment.organisation_id.0,
                            "name": deployment.name.0.clone(),
                            "kind": deployment.kind.to_string(),
                            "version": deployment.version.0.clone(),
                            "namespace": deployment.namespace.clone(),
                            "created_by": deployment.created_by.0,
                        }),
                    },
                    ActionVersion(1),
                    ActionSource::User {
                        user_id: deployment.created_by.0,
                    },
                );

                action_service.record_action(action_command).await?;

                Ok(deployment)
            })
        })
        .await
    }

    async fn delete_deployment(&self, deployment_id: DeploymentId) -> Result<(), CoreError> {
        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = std::sync::Arc::new(tokio::sync::Mutex::new(Some(tx)));

        run_deployment_transaction(tx, |tx| {
            Box::pin(async move {
                let deployment_repository = PostgresDeploymentRepository::from_tx(tx);
                let user_repository = PostgresUserRepository::from_tx(tx);
                let deployment_service =
                    DeploymentServiceImpl::new(deployment_repository, user_repository);

                deployment_service.delete_deployment(deployment_id).await
            })
        })
        .await
    }

    async fn delete_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<(), CoreError> {
        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = std::sync::Arc::new(tokio::sync::Mutex::new(Some(tx)));

        run_deployment_transaction(tx, |tx| {
            Box::pin(async move {
                let deployment_repository = PostgresDeploymentRepository::from_tx(tx);
                let user_repository = PostgresUserRepository::from_tx(tx);
                let deployment_service =
                    DeploymentServiceImpl::new(deployment_repository, user_repository);

                deployment_service
                    .delete_deployment_for_organisation(organisation_id, deployment_id)
                    .await
            })
        })
        .await
    }

    async fn get_deployment(
        &self,
        deployment_id: DeploymentId,
    ) -> Result<Option<Deployment>, CoreError> {
        let deployment_repository = PostgresDeploymentRepository::from_pool(self.pool());
        let user_repository = PostgresUserRepository::from_pool(self.pool());
        let deployment_service = DeploymentServiceImpl::new(deployment_repository, user_repository);

        deployment_service.get_deployment(deployment_id).await
    }

    async fn get_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<Deployment, CoreError> {
        let deployment_repository = PostgresDeploymentRepository::from_pool(self.pool());
        let user_repository = PostgresUserRepository::from_pool(self.pool());
        let deployment_service = DeploymentServiceImpl::new(deployment_repository, user_repository);

        deployment_service
            .get_deployment_for_organisation(organisation_id, deployment_id)
            .await
    }

    async fn list_deployments_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Deployment>, CoreError> {
        let deployment_repository = PostgresDeploymentRepository::from_pool(self.pool());
        let user_repository = PostgresUserRepository::from_pool(self.pool());
        let deployment_service = DeploymentServiceImpl::new(deployment_repository, user_repository);

        deployment_service
            .list_deployments_by_organisation(organisation_id)
            .await
    }

    async fn update_deployment(
        &self,
        deployment_id: DeploymentId,
        command: UpdateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = std::sync::Arc::new(tokio::sync::Mutex::new(Some(tx)));

        run_deployment_transaction(tx, |tx| {
            Box::pin(async move {
                let deployment_repository = PostgresDeploymentRepository::from_tx(tx);
                let user_repository = PostgresUserRepository::from_tx(tx);
                let deployment_service =
                    DeploymentServiceImpl::new(deployment_repository, user_repository);

                deployment_service
                    .update_deployment(deployment_id, command)
                    .await
            })
        })
        .await
    }

    async fn update_deployment_for_organisation(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        command: UpdateDeploymentCommand,
    ) -> Result<Deployment, CoreError> {
        let tx = self
            .pool()
            .begin()
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: e.to_string(),
            })?;
        let tx = std::sync::Arc::new(tokio::sync::Mutex::new(Some(tx)));

        run_deployment_transaction(tx, |tx| {
            Box::pin(async move {
                let deployment_repository = PostgresDeploymentRepository::from_tx(tx);
                let user_repository = PostgresUserRepository::from_tx(tx);
                let deployment_service =
                    DeploymentServiceImpl::new(deployment_repository, user_repository);

                deployment_service
                    .update_deployment_for_organisation(organisation_id, deployment_id, command)
                    .await
            })
        })
        .await
    }
}

type TxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

trait DeploymentTransaction {
    fn commit<'a>(&'a self) -> TxFuture<'a, Result<(), CoreError>>;
    fn rollback<'a>(&'a self) -> TxFuture<'a, Result<(), CoreError>>;
}

impl<'t> DeploymentTransaction
    for std::sync::Arc<tokio::sync::Mutex<Option<sqlx::Transaction<'t, sqlx::Postgres>>>>
{
    fn commit<'a>(&'a self) -> TxFuture<'a, Result<(), CoreError>> {
        Box::pin(async move {
            super::take_transaction(self)
                .await?
                .commit()
                .await
                .map_err(|e| CoreError::DatabaseError {
                    message: e.to_string(),
                })
        })
    }

    fn rollback<'a>(&'a self) -> TxFuture<'a, Result<(), CoreError>> {
        Box::pin(async move {
            super::take_transaction(self)
                .await?
                .rollback()
                .await
                .map_err(|e| CoreError::DatabaseError {
                    message: e.to_string(),
                })
        })
    }
}

async fn run_deployment_transaction<T, Tx>(
    tx: Tx,
    op: impl for<'a> FnOnce(&'a Tx) -> TxFuture<'a, Result<T, CoreError>>,
) -> Result<T, CoreError>
where
    Tx: DeploymentTransaction,
{
    let result = op(&tx).await;

    match result {
        Ok(value) => {
            tx.commit().await?;
            Ok(value)
        }
        Err(err) => {
            tx.rollback().await?;
            Err(err)
        }
    }
}

#[cfg(test)]
struct FakeDeploymentTx {
    commits: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    rollbacks: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    commit_error: bool,
    rollback_error: bool,
}

#[cfg(test)]
impl FakeDeploymentTx {
    fn new(
        commits: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        rollbacks: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) -> Self {
        Self {
            commits,
            rollbacks,
            commit_error: false,
            rollback_error: false,
        }
    }

    fn with_commit_error(mut self) -> Self {
        self.commit_error = true;
        self
    }

    fn with_rollback_error(mut self) -> Self {
        self.rollback_error = true;
        self
    }
}

#[cfg(test)]
impl DeploymentTransaction for FakeDeploymentTx {
    fn commit<'a>(&'a self) -> TxFuture<'a, Result<(), CoreError>> {
        Box::pin(async move {
            self.commits
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.commit_error {
                return Err(CoreError::InternalError("commit error".to_string()));
            }
            Ok(())
        })
    }

    fn rollback<'a>(&'a self) -> TxFuture<'a, Result<(), CoreError>> {
        Box::pin(async move {
            self.rollbacks
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.rollback_error {
                return Err(CoreError::InternalError("rollback error".to_string()));
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::deployments::{
        DeploymentKind, DeploymentName, DeploymentStatus, DeploymentVersion,
    };
    use crate::domain::user::UserId;
    use sqlx::postgres::PgPoolOptions;
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use uuid::Uuid;

    fn service() -> AetherService {
        let pool = PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(50))
            .connect_lazy("postgres://user:pass@127.0.0.1:1/db")
            .expect("valid database url");
        AetherService::new(pool)
    }

    #[tokio::test]
    async fn create_deployment_maps_pool_error() {
        let command = CreateDeploymentCommand::new(
            OrganisationId(Uuid::new_v4()),
            DeploymentName("deployment".to_string()),
            DeploymentKind::Keycloak,
            DeploymentVersion("1.0.0".to_string()),
            DeploymentStatus::Pending,
            "namespace".to_string(),
            UserId(Uuid::new_v4()),
        );

        let result = service().create_deployment(command).await;
        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn list_deployments_maps_pool_error() {
        let result = service()
            .list_deployments_by_organisation(OrganisationId(Uuid::new_v4()))
            .await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn get_deployment_maps_pool_error() {
        let result = service().get_deployment(DeploymentId(Uuid::new_v4())).await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn get_deployment_for_organisation_maps_pool_error() {
        let result = service()
            .get_deployment_for_organisation(
                OrganisationId(Uuid::new_v4()),
                DeploymentId(Uuid::new_v4()),
            )
            .await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn delete_deployment_maps_pool_error() {
        let result = service()
            .delete_deployment(DeploymentId(Uuid::new_v4()))
            .await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn delete_deployment_for_organisation_maps_pool_error() {
        let result = service()
            .delete_deployment_for_organisation(
                OrganisationId(Uuid::new_v4()),
                DeploymentId(Uuid::new_v4()),
            )
            .await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn update_deployment_maps_pool_error() {
        let command = UpdateDeploymentCommand::new()
            .with_name(DeploymentName("name".to_string()))
            .with_status(DeploymentStatus::Pending);

        let result = service()
            .update_deployment(DeploymentId(Uuid::new_v4()), command)
            .await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn update_deployment_for_organisation_maps_pool_error() {
        let command = UpdateDeploymentCommand::new()
            .with_name(DeploymentName("name".to_string()))
            .with_status(DeploymentStatus::Pending);

        let result = service()
            .update_deployment_for_organisation(
                OrganisationId(Uuid::new_v4()),
                DeploymentId(Uuid::new_v4()),
                command,
            )
            .await;

        assert!(matches!(result, Err(CoreError::DatabaseError { .. })));
    }

    #[tokio::test]
    async fn run_deployment_transaction_commits_on_ok() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        let commits = Arc::new(AtomicUsize::new(0));
        let rollbacks = Arc::new(AtomicUsize::new(0));

        let tx = FakeDeploymentTx::new(commits.clone(), rollbacks.clone());
        let result: Result<i32, CoreError> =
            run_deployment_transaction(tx, |_| Box::pin(async { Ok(42) })).await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(commits.load(Ordering::SeqCst), 1);
        assert_eq!(rollbacks.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn run_deployment_transaction_rolls_back_on_err() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        let commits = Arc::new(AtomicUsize::new(0));
        let rollbacks = Arc::new(AtomicUsize::new(0));

        let tx = FakeDeploymentTx::new(commits.clone(), rollbacks.clone());
        let result: Result<i32, CoreError> = run_deployment_transaction(tx, |_| {
            Box::pin(async { Err(CoreError::InternalError("fail".to_string())) })
        })
        .await;

        assert!(matches!(result, Err(CoreError::InternalError(_))));
        assert_eq!(commits.load(Ordering::SeqCst), 0);
        assert_eq!(rollbacks.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn run_deployment_transaction_commit_error() {
        let commits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let rollbacks = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let tx = FakeDeploymentTx::new(commits.clone(), rollbacks.clone()).with_commit_error();
        let result: Result<i32, CoreError> =
            run_deployment_transaction(tx, |_| Box::pin(async { Ok(1) })).await;

        assert!(matches!(result, Err(CoreError::InternalError(_))));
        assert_eq!(commits.load(Ordering::SeqCst), 1);
        assert_eq!(rollbacks.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn run_deployment_transaction_rollback_error() {
        let commits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let rollbacks = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let tx = FakeDeploymentTx::new(commits.clone(), rollbacks.clone()).with_rollback_error();
        let result: Result<i32, CoreError> = run_deployment_transaction(tx, |_| {
            Box::pin(async { Err(CoreError::InternalError("fail".to_string())) })
        })
        .await;

        assert!(matches!(result, Err(CoreError::InternalError(_))));
        assert_eq!(commits.load(Ordering::SeqCst), 0);
        assert_eq!(rollbacks.load(Ordering::SeqCst), 1);
    }
}
