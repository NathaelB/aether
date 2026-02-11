use std::sync::Arc;
use std::time::Duration;

use aether_crds::common::types::Phase;
use aether_crds::v1alpha::identity_instance::{IdentityInstance, IdentityInstanceStatus};

use crate::domain::ports::{
    IdentityInstanceDeployer, IdentityInstanceRepository, IdentityInstanceService,
};
use crate::domain::{OperatorError, ReconcileOutcome};

const DEPLOYING_REQUEUE_SECONDS: u64 = 15;
const STEADY_STATE_REQUEUE_SECONDS: u64 = 60;

pub struct IdentityInstanceServiceImpl<R, D> {
    repository: Arc<R>,
    deployer: Arc<D>,
}

impl<R, D> IdentityInstanceServiceImpl<R, D> {
    pub fn new(repository: Arc<R>, deployer: Arc<D>) -> Self {
        Self {
            repository,
            deployer,
        }
    }

    fn build_desired_status(
        &self,
        instance: &IdentityInstance,
        database_ready: bool,
        provider_ready: bool,
        upgrade_in_progress: bool,
    ) -> IdentityInstanceStatus {
        let mut status = instance.status.clone().unwrap_or_default();

        status.ready = database_ready && provider_ready && !upgrade_in_progress;
        status.phase = Some(if !database_ready {
            Phase::DatabaseProvisioning
        } else if upgrade_in_progress {
            Phase::Upgrading
        } else if provider_ready {
            Phase::Running
        } else {
            Phase::Deploying
        });

        if status.endpoint.is_none() {
            status.endpoint = Some(format!("https://{}", instance.spec.hostname));
        }

        if status.admin_url.is_none() {
            status.admin_url = Some(format!("https://{}/admin", instance.spec.hostname));
        }

        status
    }
}

impl<R, D> IdentityInstanceService for IdentityInstanceServiceImpl<R, D>
where
    R: IdentityInstanceRepository,
    D: IdentityInstanceDeployer,
{
    async fn reconcile(
        &self,
        instance: IdentityInstance,
    ) -> Result<ReconcileOutcome, OperatorError> {
        self.ensure_instance(&instance).await?;
        let database_ready = self.deployer.database_ready(&instance).await?;
        let provider_ready = self.deployer.provider_ready(&instance).await?;
        let upgrade_in_progress = self.deployer.upgrade_in_progress(&instance).await?;
        let current_status = instance.status.clone().unwrap_or_default();
        let desired_status = self.build_desired_status(
            &instance,
            database_ready,
            provider_ready,
            upgrade_in_progress,
        );

        if desired_status != current_status {
            self.repository
                .patch_status(&instance, desired_status)
                .await?;
            return Ok(ReconcileOutcome::requeue_after(Duration::from_secs(
                DEPLOYING_REQUEUE_SECONDS,
            )));
        }

        if !database_ready || !provider_ready || upgrade_in_progress {
            return Ok(ReconcileOutcome::requeue_after(Duration::from_secs(
                DEPLOYING_REQUEUE_SECONDS,
            )));
        }

        Ok(ReconcileOutcome::requeue_after(Duration::from_secs(
            STEADY_STATE_REQUEUE_SECONDS,
        )))
    }
}

impl<R, D> IdentityInstanceServiceImpl<R, D>
where
    D: IdentityInstanceDeployer,
{
    async fn ensure_instance(&self, instance: &IdentityInstance) -> Result<(), OperatorError> {
        self.deployer.ensure_provider_resources(instance).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ports::{MockIdentityInstanceDeployer, MockIdentityInstanceRepository};
    use aether_crds::common::types::Phase;
    use aether_crds::common::types::ResourceRequirements;
    use aether_crds::v1alpha::identity_instance::{
        DatabaseConfig, DatabaseMode, IdentityInstance, IdentityInstanceSpec,
        IdentityInstanceStatus, IdentityProvider, ManagedClusterConfig, ManagedClusterStorage,
    };
    use kube::core::ObjectMeta;
    use std::sync::Arc;

    fn instance_with_status(status: Option<IdentityInstanceStatus>) -> IdentityInstance {
        IdentityInstance {
            metadata: ObjectMeta {
                name: Some("instance-1".to_string()),
                namespace: Some("default".to_string()),
                ..Default::default()
            },
            spec: IdentityInstanceSpec {
                organisation_id: "org-1".to_string(),
                provider: IdentityProvider::Keycloak,
                version: "25.0.0".to_string(),
                hostname: "auth.acme.test".to_string(),
                database: DatabaseConfig {
                    mode: DatabaseMode::ManagedCluster,
                    managed_cluster: ManagedClusterConfig {
                        instances: 1,
                        storage: ManagedClusterStorage {
                            size: "10Gi".to_string(),
                            storage_class: None,
                        },
                        resources: ResourceRequirements {
                            requests: None,
                            limits: None,
                        },
                    },
                },
            },
            status,
        }
    }

    #[tokio::test]
    async fn reconcile_updates_status_and_requeues() {
        let instance = instance_with_status(None);
        let mut repository = MockIdentityInstanceRepository::new();
        let mut deployer = MockIdentityInstanceDeployer::new();

        deployer
            .expect_ensure_provider_resources()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        deployer
            .expect_database_ready()
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));
        deployer
            .expect_provider_ready()
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));
        deployer
            .expect_upgrade_in_progress()
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));

        repository
            .expect_patch_status()
            .times(1)
            .withf(|instance, status| {
                instance.metadata.name.as_deref() == Some("instance-1")
                    && status.phase == Some(Phase::DatabaseProvisioning)
                    && !status.ready
                    && status.endpoint.as_deref() == Some("https://auth.acme.test")
                    && status.admin_url.as_deref() == Some("https://auth.acme.test/admin")
            })
            .returning(|instance, _status| {
                let instance = instance.clone();
                Box::pin(async move { Ok(instance) })
            });

        let service = IdentityInstanceServiceImpl::new(Arc::new(repository), Arc::new(deployer));
        let outcome = service.reconcile(instance).await.unwrap();

        assert_eq!(outcome.requeue_after, Some(Duration::from_secs(15)));
    }

    #[tokio::test]
    async fn reconcile_no_status_change_does_not_patch() {
        let status = IdentityInstanceStatus {
            phase: Some(Phase::Running),
            ready: true,
            endpoint: Some("https://auth.acme.test".to_string()),
            admin_url: Some("https://auth.acme.test/admin".to_string()),
            ..Default::default()
        };
        let instance = instance_with_status(Some(status));
        let mut repository = MockIdentityInstanceRepository::new();
        let mut deployer = MockIdentityInstanceDeployer::new();

        deployer
            .expect_ensure_provider_resources()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        deployer
            .expect_database_ready()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        deployer
            .expect_provider_ready()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        deployer
            .expect_upgrade_in_progress()
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));
        repository.expect_patch_status().times(0);

        let service = IdentityInstanceServiceImpl::new(Arc::new(repository), Arc::new(deployer));
        let outcome = service.reconcile(instance).await.unwrap();

        assert_eq!(
            outcome.requeue_after,
            Some(Duration::from_secs(STEADY_STATE_REQUEUE_SECONDS))
        );
    }

    #[tokio::test]
    async fn reconcile_requeues_while_provider_not_ready_even_without_status_change() {
        let status = IdentityInstanceStatus {
            phase: Some(Phase::Deploying),
            ready: false,
            endpoint: Some("https://auth.acme.test".to_string()),
            admin_url: Some("https://auth.acme.test/admin".to_string()),
            ..Default::default()
        };
        let instance = instance_with_status(Some(status));
        let mut repository = MockIdentityInstanceRepository::new();
        let mut deployer = MockIdentityInstanceDeployer::new();

        deployer
            .expect_ensure_provider_resources()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        deployer
            .expect_database_ready()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        deployer
            .expect_provider_ready()
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));
        deployer
            .expect_upgrade_in_progress()
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));
        repository.expect_patch_status().times(0);

        let service = IdentityInstanceServiceImpl::new(Arc::new(repository), Arc::new(deployer));
        let outcome = service.reconcile(instance).await.unwrap();

        assert_eq!(
            outcome.requeue_after,
            Some(Duration::from_secs(DEPLOYING_REQUEUE_SECONDS))
        );
    }

    #[test]
    fn build_desired_status_keeps_existing_fields() {
        let status = IdentityInstanceStatus {
            phase: Some(Phase::Running),
            ready: true,
            endpoint: Some("https://already.example".to_string()),
            admin_url: Some("https://already.example/admin".to_string()),
            ..Default::default()
        };
        let instance = instance_with_status(Some(status.clone()));
        let service = IdentityInstanceServiceImpl::new(
            Arc::new(MockIdentityInstanceRepository::new()),
            Arc::new(MockIdentityInstanceDeployer::new()),
        );

        let desired = service.build_desired_status(&instance, true, true, false);
        assert_eq!(desired.phase, status.phase);
        assert_eq!(desired.ready, status.ready);
        assert_eq!(desired.endpoint, status.endpoint);
        assert_eq!(desired.admin_url, status.admin_url);
    }
}
