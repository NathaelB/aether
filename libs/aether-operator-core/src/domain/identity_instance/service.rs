use std::sync::Arc;
use std::time::Duration;

use aether_crds::common::types::Phase;
use aether_crds::v1alpha::identity_instance::{IdentityInstance, IdentityInstanceStatus};

use crate::domain::ports::{
    IdentityInstanceDeployer, IdentityInstanceRepository, IdentityInstanceService,
};
use crate::domain::{OperatorError, ReconcileOutcome};

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

    fn build_desired_status(&self, instance: &IdentityInstance) -> IdentityInstanceStatus {
        let mut status = instance.status.clone().unwrap_or_default();

        if status.phase.is_none() {
            status.phase = Some(Phase::Pending);
        }

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
        let current_status = instance.status.clone().unwrap_or_default();
        let desired_status = self.build_desired_status(&instance);

        if desired_status != current_status {
            self.repository
                .patch_status(&instance, desired_status)
                .await?;
            return Ok(ReconcileOutcome::requeue_after(Duration::from_secs(15)));
        }

        Ok(ReconcileOutcome::default())
    }
}

impl<R, D> IdentityInstanceServiceImpl<R, D>
where
    D: IdentityInstanceDeployer,
{
    async fn ensure_instance(&self, instance: &IdentityInstance) -> Result<(), OperatorError> {
        match instance.spec.provider {
            aether_crds::v1alpha::identity_instance::IdentityProvider::Keycloak => {
                self.deployer.ensure_keycloak_resources(instance).await?;
            }
            aether_crds::v1alpha::identity_instance::IdentityProvider::Ferriskey => {
                // TODO: creer l'instance Ferriskey (DB geree par l'operateur CNPG).
                unimplemented!("Ferriskey instance creation not implemented yet");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ports::{MockIdentityInstanceDeployer, MockIdentityInstanceRepository};
    use aether_crds::common::types::Phase;
    use aether_crds::v1alpha::identity_instance::{
        DatabaseConfig, IdentityInstance, IdentityInstanceSpec, IdentityInstanceStatus,
        IdentityProvider,
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
                    host: "postgres.default.svc".to_string(),
                    port: 5432,
                    name: "keycloak_acme".to_string(),
                    credentials_secret: "db-creds".to_string(),
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
            .expect_ensure_keycloak_resources()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));

        repository
            .expect_patch_status()
            .times(1)
            .withf(|instance, status| {
                instance.metadata.name.as_deref() == Some("instance-1")
                    && status.phase == Some(Phase::Pending)
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
            phase: Some(Phase::Pending),
            endpoint: Some("https://auth.acme.test".to_string()),
            admin_url: Some("https://auth.acme.test/admin".to_string()),
            ..Default::default()
        };
        let instance = instance_with_status(Some(status));
        let mut repository = MockIdentityInstanceRepository::new();
        let mut deployer = MockIdentityInstanceDeployer::new();

        deployer
            .expect_ensure_keycloak_resources()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        repository.expect_patch_status().times(0);

        let service = IdentityInstanceServiceImpl::new(Arc::new(repository), Arc::new(deployer));
        let outcome = service.reconcile(instance).await.unwrap();

        assert!(outcome.requeue_after.is_none());
    }

    #[test]
    fn build_desired_status_keeps_existing_fields() {
        let status = IdentityInstanceStatus {
            phase: Some(Phase::Running),
            endpoint: Some("https://already.example".to_string()),
            admin_url: Some("https://already.example/admin".to_string()),
            ..Default::default()
        };
        let instance = instance_with_status(Some(status.clone()));
        let service = IdentityInstanceServiceImpl::new(
            Arc::new(MockIdentityInstanceRepository::new()),
            Arc::new(MockIdentityInstanceDeployer::new()),
        );

        let desired = service.build_desired_status(&instance);
        assert_eq!(desired.phase, status.phase);
        assert_eq!(desired.endpoint, status.endpoint);
        assert_eq!(desired.admin_url, status.admin_url);
    }

    #[tokio::test]
    #[should_panic(expected = "Ferriskey instance creation not implemented yet")]
    async fn ensure_instance_panics_for_ferriskey() {
        let mut instance = instance_with_status(None);
        instance.spec.provider = IdentityProvider::Ferriskey;

        let service = IdentityInstanceServiceImpl::new(
            Arc::new(MockIdentityInstanceRepository::new()),
            Arc::new(MockIdentityInstanceDeployer::new()),
        );

        let _ = service.ensure_instance(&instance).await;
    }
}
