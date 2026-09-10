use aether_crds::v1alpha::identity_instance_upgrade::{
    IdentityInstanceRef as CrdInstanceRef, IdentityInstanceUpgrade, IdentityInstanceUpgradeSpec,
    UpgradeStrategy,
};
use kube::api::PostParams;
use kube::core::ObjectMeta;
use kube::{Api, Client};
use tracing::info;

use crate::domain::entities::identity_instance::{DesiredUpgrade, InFlightUpgrade, UpgradeRef};
use crate::domain::error::GenesisError;
use crate::domain::ports::{BoxFuture, IdentityInstanceUpgradePort};

pub struct KubeIdentityInstanceUpgradePort {
    client: Client,
}

impl KubeIdentityInstanceUpgradePort {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn from_env() -> Result<Self, GenesisError> {
        let client = Client::try_default()
            .await
            .map_err(|error| GenesisError::Kubernetes {
                message: format!("failed to build Kubernetes client: {error}"),
            })?;

        Ok(Self::new(client))
    }
}

impl IdentityInstanceUpgradePort for KubeIdentityInstanceUpgradePort {
    fn find<'a>(
        &'a self,
        reference: &'a UpgradeRef,
    ) -> BoxFuture<'a, Result<Option<InFlightUpgrade>, GenesisError>> {
        Box::pin(async move {
            let upgrades: Api<IdentityInstanceUpgrade> =
                Api::namespaced(self.client.clone(), &reference.namespace);

            let found = upgrades.get_opt(&reference.name).await.map_err(|error| {
                GenesisError::Kubernetes {
                    message: format!("failed to read upgrade '{}': {error}", reference.name),
                }
            })?;

            Ok(found.map(|upgrade| InFlightUpgrade {
                target_version: upgrade.spec.target_version,
            }))
        })
    }

    fn create<'a>(
        &'a self,
        desired: &'a DesiredUpgrade,
    ) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async move {
            let upgrades: Api<IdentityInstanceUpgrade> =
                Api::namespaced(self.client.clone(), &desired.namespace);

            let upgrade = IdentityInstanceUpgrade {
                metadata: ObjectMeta {
                    name: Some(desired.name.clone()),
                    namespace: Some(desired.namespace.clone()),
                    ..Default::default()
                },
                spec: IdentityInstanceUpgradeSpec {
                    identity_instance_ref: CrdInstanceRef {
                        name: desired.instance_name.clone(),
                    },
                    target_version: desired.target_version.clone(),
                    strategy: UpgradeStrategy::Rolling,
                    // The control plane has already decided. Its own approval
                    // gate is what the client policy will use in V4; a second
                    // gate here would be one nobody ever opens.
                    approved: true,
                },
                status: None,
            };

            match upgrades.create(&PostParams::default(), &upgrade).await {
                Ok(_) => {
                    info!(
                        name = %desired.name,
                        namespace = %desired.namespace,
                        target = %desired.target_version,
                        "created the upgrade"
                    );
                    Ok(())
                }
                // Created between the find and the create. The find already
                // decided this step is the right one, so losing that race is
                // not a failure: the resource that won says the same thing.
                Err(kube::Error::Api(response)) if response.code == 409 => {
                    info!(
                        name = %desired.name,
                        namespace = %desired.namespace,
                        "the upgrade was created concurrently; leaving it alone"
                    );
                    Ok(())
                }
                Err(error) => Err(GenesisError::Kubernetes {
                    message: format!("failed to create upgrade '{}': {error}", desired.name),
                }),
            }
        })
    }
}
