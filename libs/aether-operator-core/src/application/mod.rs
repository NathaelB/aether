use std::sync::Arc;

use aether_crds::v1alpha::identity_instance::IdentityInstance;

use crate::domain::identity_instance::IdentityInstanceServiceImpl;
use crate::domain::ports::{
    IdentityInstanceDeployer, IdentityInstanceRepository, IdentityInstanceService,
};
use crate::domain::{OperatorError, ReconcileOutcome};

pub struct OperatorApplication<R, D> {
    pub identity_instance_service: IdentityInstanceServiceImpl<R, D>,
}

impl<R, D> OperatorApplication<R, D> {
    pub fn new(repository: Arc<R>, deployer: Arc<D>) -> Self {
        Self {
            identity_instance_service: IdentityInstanceServiceImpl::new(repository, deployer),
        }
    }
}

impl<R, D> IdentityInstanceService for OperatorApplication<R, D>
where
    R: IdentityInstanceRepository,
    D: IdentityInstanceDeployer,
{
    async fn reconcile(
        &self,
        instance: IdentityInstance,
    ) -> Result<ReconcileOutcome, OperatorError> {
        self.identity_instance_service.reconcile(instance).await
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
    async fn reconcile_delegates_to_service_impl() {
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
            .withf(|_instance, status| status.phase == Some(Phase::Pending))
            .returning(|instance, _status| {
                let instance = instance.clone();
                Box::pin(async move { Ok(instance) })
            });

        let app = OperatorApplication::new(Arc::new(repository), Arc::new(deployer));
        let outcome = app.reconcile(instance).await.unwrap();

        assert_eq!(
            outcome.requeue_after,
            Some(std::time::Duration::from_secs(15))
        );
    }
}
