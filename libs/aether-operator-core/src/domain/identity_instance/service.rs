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
