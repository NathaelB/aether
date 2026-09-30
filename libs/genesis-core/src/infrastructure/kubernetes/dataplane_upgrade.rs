use aether_crds::v1alpha::identity_dataplane_upgrade::{
    DataplaneComponent, DataplaneUpgradePhase, DataplaneUpgradeStrategy, IdentityDataplaneUpgrade,
    IdentityDataplaneUpgradeSpec,
};
use kube::api::{DeleteParams, PostParams};
use kube::core::ObjectMeta;
use kube::{Api, Client};
use tracing::info;

use crate::domain::entities::dataplane_upgrade::{
    DataplaneUpgradeRef, DesiredDataplaneUpgrade, ExistingDataplaneUpgrade,
};
use crate::domain::entities::dataplane_upgrade_payload::{UpgradeComponent, UpgradeStrategy};
use crate::domain::error::GenesisError;
use crate::domain::ports::{BoxFuture, DataplaneUpgradePort};

pub struct KubeDataplaneUpgradePort {
    client: Client,
}

impl KubeDataplaneUpgradePort {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    fn api(&self, namespace: &str) -> Api<IdentityDataplaneUpgrade> {
        Api::namespaced(self.client.clone(), namespace)
    }
}

fn component(component: UpgradeComponent) -> DataplaneComponent {
    match component {
        UpgradeComponent::Herald => DataplaneComponent::Herald,
        UpgradeComponent::Genesis => DataplaneComponent::Genesis,
        UpgradeComponent::Operator => DataplaneComponent::Operator,
        UpgradeComponent::All => DataplaneComponent::All,
    }
}

fn strategy(strategy: UpgradeStrategy) -> DataplaneUpgradeStrategy {
    match strategy {
        UpgradeStrategy::Rolling => DataplaneUpgradeStrategy::Rolling,
        UpgradeStrategy::Canary => DataplaneUpgradeStrategy::Canary,
    }
}

fn resource(desired: &DesiredDataplaneUpgrade) -> IdentityDataplaneUpgrade {
    IdentityDataplaneUpgrade {
        metadata: ObjectMeta {
            name: Some(desired.name.clone()),
            namespace: Some(desired.namespace.clone()),
            ..Default::default()
        },
        spec: IdentityDataplaneUpgradeSpec {
            dataplane_id: desired.dataplane_id.clone(),
            target_version: desired.target_version.clone(),
            components: desired.components.iter().copied().map(component).collect(),
            strategy: strategy(desired.strategy),
            max_unavailable: desired.max_unavailable,
        },
        status: None,
    }
}

fn is_finished(upgrade: &IdentityDataplaneUpgrade) -> bool {
    upgrade.status.as_ref().is_some_and(|status| {
        matches!(
            status.phase,
            DataplaneUpgradePhase::Completed
                | DataplaneUpgradePhase::Failed
                | DataplaneUpgradePhase::RolledBack
        )
    })
}

fn is_succeeded(upgrade: &IdentityDataplaneUpgrade) -> bool {
    upgrade
        .status
        .as_ref()
        .is_some_and(|status| matches!(status.phase, DataplaneUpgradePhase::Completed))
}

fn failure(action: &str, name: &str, error: kube::Error) -> GenesisError {
    GenesisError::Kubernetes {
        message: format!("failed to {action} data plane upgrade '{name}': {error}"),
    }
}

impl DataplaneUpgradePort for KubeDataplaneUpgradePort {
    fn find<'a>(
        &'a self,
        reference: &'a DataplaneUpgradeRef,
    ) -> BoxFuture<'a, Result<Option<ExistingDataplaneUpgrade>, GenesisError>> {
        Box::pin(async move {
            let found = self
                .api(&reference.namespace)
                .get_opt(&reference.name)
                .await
                .map_err(|error| failure("read", &reference.name, error))?;

            Ok(found.map(|upgrade| ExistingDataplaneUpgrade {
                finished: is_finished(&upgrade),
                succeeded: is_succeeded(&upgrade),
                target_version: upgrade.spec.target_version,
            }))
        })
    }

    fn create<'a>(
        &'a self,
        desired: &'a DesiredDataplaneUpgrade,
    ) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async move {
            match self
                .api(&desired.namespace)
                .create(&PostParams::default(), &resource(desired))
                .await
            {
                Ok(_) => {
                    info!(
                        name = %desired.name,
                        namespace = %desired.namespace,
                        target = %desired.target_version,
                        "created the data plane upgrade"
                    );
                    Ok(())
                }
                Err(kube::Error::Api(response)) if response.code == 409 => {
                    info!(
                        name = %desired.name,
                        namespace = %desired.namespace,
                        "the data plane upgrade was created concurrently; leaving it alone"
                    );
                    Ok(())
                }
                Err(error) => Err(failure("create", &desired.name, error)),
            }
        })
    }

    fn replace<'a>(
        &'a self,
        desired: &'a DesiredDataplaneUpgrade,
    ) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async move {
            let upgrades = self.api(&desired.namespace);

            match upgrades
                .delete(&desired.name, &DeleteParams::default())
                .await
            {
                Ok(_) => {}
                Err(kube::Error::Api(response)) if response.code == 404 => {}
                Err(error) => return Err(failure("delete", &desired.name, error)),
            }

            upgrades
                .create(&PostParams::default(), &resource(desired))
                .await
                .map_err(|error| failure("recreate", &desired.name, error))?;

            info!(
                name = %desired.name,
                namespace = %desired.namespace,
                target = %desired.target_version,
                "replaced the finished data plane upgrade"
            );
            Ok(())
        })
    }
}
