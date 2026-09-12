use std::collections::BTreeMap;

use aether_crds::common::types::{ResourceList, ResourceRequirements};
use aether_crds::v1alpha::identity_instance::{
    DatabaseConfig, DatabaseMode, FerriskeyConfig, IdentityInstance, IdentityInstanceSpec,
    IdentityProvider, ManagedClusterConfig, ManagedClusterStorage,
};
use k8s_openapi::api::core::v1::Namespace;
use kube::api::{DeleteParams, Patch, PatchParams};
use kube::core::ObjectMeta;
use kube::{Api, Client};
use tracing::info;

use crate::domain::entities::identity_instance::{
    DesiredIdentityInstance, IdentityInstanceProvider, IdentityInstanceRef,
};
use crate::domain::error::GenesisError;
use crate::domain::ports::{BoxFuture, IdentityInstancePort};

/// The field manager genesis identifies itself as when server-side-applying resources.
/// Using a stable manager name (rather than a blind create) is what makes redelivering
/// the same `deployment.*` event safe: the second apply simply re-asserts the same
/// fields instead of conflicting with a prior create.
const FIELD_MANAGER: &str = "genesis";

pub struct KubeIdentityInstancePort {
    client: Client,
}

impl KubeIdentityInstancePort {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Builds a client from the ambient Kubernetes configuration (in-cluster config
    /// when running as a pod, or the local kubeconfig otherwise).
    pub async fn from_env() -> Result<Self, GenesisError> {
        let client = Client::try_default()
            .await
            .map_err(|error| GenesisError::Kubernetes {
                message: format!("failed to build Kubernetes client: {error}"),
            })?;

        Ok(Self::new(client))
    }
}

impl KubeIdentityInstancePort {
    /// Creates the namespace the deployment lives in, if it is not there.
    ///
    /// Nobody else does. The control plane derives a namespace name from the
    /// environment and the deployment name and sends it in the payload; the
    /// operator reconciles resources *inside* it; and Kubernetes will not
    /// create one implicitly. So without this the apply below fails with
    /// `namespaces "..." not found` -- forever, for every deployment.
    ///
    /// Server-side apply rather than create-and-ignore-409: the same event can
    /// be redelivered, and an apply that re-asserts the same fields is the
    /// whole reason this adapter uses a stable field manager.
    async fn ensure_namespace(&self, namespace: &str) -> Result<(), GenesisError> {
        let api: Api<Namespace> = Api::all(self.client.clone());
        let params = PatchParams::apply(FIELD_MANAGER).force();

        let resource = Namespace {
            metadata: ObjectMeta {
                name: Some(namespace.to_string()),
                // So an operator can tell which namespaces on a data plane are
                // Aether's, and a cleanup can find them without guessing at
                // the naming convention.
                labels: Some(BTreeMap::from([(
                    "app.kubernetes.io/managed-by".to_string(),
                    FIELD_MANAGER.to_string(),
                )])),
                ..Default::default()
            },
            ..Default::default()
        };

        info!(namespace = %namespace, "ensuring namespace");

        api.patch(namespace, &params, &Patch::Apply(&resource))
            .await
            .map_err(|error| GenesisError::Kubernetes {
                message: format!("failed to ensure namespace {namespace}: {error}"),
            })?;

        Ok(())
    }
}

impl IdentityInstancePort for KubeIdentityInstancePort {
    fn apply<'a>(
        &'a self,
        desired: &'a DesiredIdentityInstance,
    ) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async move {
            self.ensure_namespace(&desired.reference.namespace).await?;

            let api: Api<IdentityInstance> =
                Api::namespaced(self.client.clone(), &desired.reference.namespace);

            let resource = to_identity_instance(desired);
            let params = PatchParams::apply(FIELD_MANAGER).force();

            info!(
                name = %desired.reference.name,
                namespace = %desired.reference.namespace,
                "applying IdentityInstance"
            );

            api.patch(&desired.reference.name, &params, &Patch::Apply(&resource))
                .await
                .map_err(|error| GenesisError::Kubernetes {
                    message: error.to_string(),
                })?;

            Ok(())
        })
    }

    fn delete<'a>(
        &'a self,
        reference: &'a IdentityInstanceRef,
    ) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async move {
            let api: Api<IdentityInstance> =
                Api::namespaced(self.client.clone(), &reference.namespace);

            info!(
                name = %reference.name,
                namespace = %reference.namespace,
                "deleting IdentityInstance"
            );

            if let Err(error) = api.delete(&reference.name, &DeleteParams::default()).await
                && !is_not_found(&error)
            {
                return Err(GenesisError::Kubernetes {
                    message: error.to_string(),
                });
            }

            Ok(())
        })
    }

    fn set_allowed_cidrs<'a>(
        &'a self,
        reference: &'a IdentityInstanceRef,
        ranges: Option<Vec<String>>,
    ) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async move {
            let api: Api<IdentityInstance> =
                Api::namespaced(self.client.clone(), &reference.namespace);

            info!(
                name = %reference.name,
                namespace = %reference.namespace,
                "setting the allow list"
            );

            // A merge patch writes null as "remove this field", which is what
            // going back to open has to mean. A server-side apply of the same
            // shape would leave the previous list in place, because an apply
            // does not remove what it does not mention.
            let patch = serde_json::json!({
                "spec": { "allowedCidrs": ranges }
            });

            api.patch(
                &reference.name,
                &kube::api::PatchParams::default(),
                &kube::api::Patch::Merge(&patch),
            )
            .await
            .map_err(|error| GenesisError::Kubernetes {
                message: error.to_string(),
            })?;

            Ok(())
        })
    }
}

fn is_not_found(error: &kube::Error) -> bool {
    matches!(error, kube::Error::Api(api_error) if api_error.code == 404)
}

fn to_identity_instance(desired: &DesiredIdentityInstance) -> IdentityInstance {
    let provider = match desired.provider {
        IdentityInstanceProvider::Keycloak => IdentityProvider::Keycloak,
        IdentityInstanceProvider::Ferriskey => IdentityProvider::Ferriskey,
    };

    let ferriskey =
        matches!(desired.provider, IdentityInstanceProvider::Ferriskey).then(|| FerriskeyConfig {
            webapp_url: None,
            api_base_url: None,
        });

    let spec = IdentityInstanceSpec {
        organisation_id: desired.organisation_id.clone(),
        provider,
        version: desired.version.clone(),
        hostname: desired.hostname.clone(),
        database: DatabaseConfig {
            mode: DatabaseMode::ManagedCluster,
            managed_cluster: ManagedClusterConfig {
                instances: desired.database.instances,
                storage: ManagedClusterStorage {
                    size: desired.database.storage_size.clone(),
                    storage_class: None,
                },
                resources: ResourceRequirements {
                    requests: Some(ResourceList {
                        cpu: Some(desired.database.cpu_request.clone()),
                        memory: Some(desired.database.memory_request.clone()),
                    }),
                    limits: Some(ResourceList {
                        cpu: Some(desired.database.cpu_limit.clone()),
                        memory: Some(desired.database.memory_limit.clone()),
                    }),
                },
            },
        },
        ferriskey,
        ingress: None,
        allowed_cidrs: None,
        // Not carried on the deployment payload yet. The control plane owns the
        // archive layout and has to send it; until it does, an instance created
        // through the API archives nowhere and says so in the operator's logs.
        backup: None,
    };

    IdentityInstance {
        metadata: ObjectMeta {
            name: Some(desired.reference.name.clone()),
            namespace: Some(desired.reference.namespace.clone()),
            ..Default::default()
        },
        spec,
        status: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::identity_instance::{DesiredDatabase, IdentityInstanceRef};
    use kube::error::ErrorResponse;

    fn desired(provider: IdentityInstanceProvider) -> DesiredIdentityInstance {
        DesiredIdentityInstance {
            reference: IdentityInstanceRef {
                name: "deployment-b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d".to_string(),
                namespace: "aether-acme-prod".to_string(),
            },
            organisation_id: "9f8e7d6c-5b4a-3c2d-1e0f-a1b2c3d4e5f6".to_string(),
            provider,
            version: "25.0.0".to_string(),
            hostname: "acme-prod.aether-acme-prod.aether.local".to_string(),
            database: DesiredDatabase::from_reserved(500, 1024, 1),
        }
    }

    #[test]
    fn maps_keycloak_desired_state_without_a_ferriskey_block() {
        let resource = to_identity_instance(&desired(IdentityInstanceProvider::Keycloak));

        assert_eq!(
            resource.metadata.name.as_deref(),
            Some("deployment-b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d")
        );
        assert_eq!(
            resource.metadata.namespace.as_deref(),
            Some("aether-acme-prod")
        );
        assert_eq!(resource.spec.provider, IdentityProvider::Keycloak);
        assert_eq!(
            resource.spec.hostname,
            "acme-prod.aether-acme-prod.aether.local"
        );
        assert!(resource.spec.ferriskey.is_none());
    }

    #[test]
    fn maps_ferriskey_desired_state_with_a_ferriskey_block() {
        let resource = to_identity_instance(&desired(IdentityInstanceProvider::Ferriskey));

        assert_eq!(resource.spec.provider, IdentityProvider::Ferriskey);
        assert!(resource.spec.ferriskey.is_some());
    }

    #[test]
    fn not_found_is_recognized_from_a_404_api_error() {
        let not_found = kube::Error::Api(ErrorResponse {
            status: "Failure".to_string(),
            message: "identityinstances.aether.dev \"x\" not found".to_string(),
            reason: "NotFound".to_string(),
            code: 404,
        });
        let conflict = kube::Error::Api(ErrorResponse {
            status: "Failure".to_string(),
            message: "conflict".to_string(),
            reason: "Conflict".to_string(),
            code: 409,
        });

        assert!(is_not_found(&not_found));
        assert!(!is_not_found(&conflict));
    }
}
