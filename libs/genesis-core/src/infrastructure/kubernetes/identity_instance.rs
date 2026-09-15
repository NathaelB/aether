use std::collections::BTreeMap;

use aether_crds::common::types::{ResourceList, ResourceRequirements};
use aether_crds::v1alpha::identity_instance::{
    BackupConfig, DatabaseConfig, DatabaseMode, FerriskeyConfig, IdentityInstance,
    IdentityInstanceSpec, IdentityProvider, ManagedClusterConfig, ManagedClusterStorage,
    RestoreConfig,
};
use aether_crds::v1alpha::identity_instance_backup::{
    IdentityInstanceBackup, IdentityInstanceBackupSchedule, IdentityInstanceBackupScheduleSpec,
    IdentityInstanceBackupSpec,
};
use aether_crds::v1alpha::identity_instance_upgrade::IdentityInstanceRef as CrdIdentityInstanceRef;
use k8s_openapi::api::core::v1::Namespace;
use kube::api::{DeleteParams, Patch, PatchParams};
use kube::core::ObjectMeta;
use kube::{Api, Client};
use tracing::info;

use crate::domain::entities::identity_instance::{
    DesiredArchive, DesiredIdentityInstance, IdentityInstanceProvider, IdentityInstanceRef,
};
use crate::domain::error::GenesisError;
use crate::domain::ports::{BoxFuture, IdentityInstancePort};

/// The field manager genesis identifies itself as when server-side-applying resources.
/// Using a stable manager name (rather than a blind create) is what makes redelivering
/// the same `deployment.*` event safe: the second apply simply re-asserts the same
/// fields instead of conflicting with a prior create.
const FIELD_MANAGER: &str = "genesis";

/// The secret CloudNativePG reads the object store credentials from, in the
/// instance's own namespace.
///
/// A fixed name rather than a configured one. Both ends of it are written by
/// this platform -- genesis names it here, the operator fills it -- and a name
/// that can differ between them is a name that eventually does.
const ARCHIVE_CREDENTIALS_SECRET: &str = "aether-object-store";

/// Where the object store answers, from inside this data plane.
///
/// Read here rather than sent by the control plane: the two sides can be on
/// different networks, and the endpoint that works from one is not always the
/// one that works from the other. Absent means AWS, which is the only store
/// that needs no endpoint.
fn store_endpoint() -> Option<String> {
    std::env::var("OBJECT_STORE_ENDPOINT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

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
    /// Applies the recurring archive beside the instance it belongs to.
    ///
    /// One schedule per instance, named after it: two would be two answers to
    /// "when is this backed up", and a generated name would leave the previous
    /// one behind on every apply.
    async fn apply_archive_schedule(
        &self,
        reference: &IdentityInstanceRef,
        archive: &DesiredArchive,
    ) -> Result<(), GenesisError> {
        let api: Api<IdentityInstanceBackupSchedule> =
            Api::namespaced(self.client.clone(), &reference.namespace);

        info!(
            name = %reference.name,
            namespace = %reference.namespace,
            "applying IdentityInstanceBackupSchedule"
        );

        let resource = to_archive_schedule(reference, archive);

        api.patch(
            &reference.name,
            &PatchParams::apply(FIELD_MANAGER).force(),
            &Patch::Apply(&resource),
        )
        .await
        .map_err(|error| GenesisError::Kubernetes {
            message: error.to_string(),
        })?;

        Ok(())
    }

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

            // After the instance, never before: a schedule referring to an
            // instance that is not there yet reconciles into an error the
            // operator retries until the instance appears, and the noise makes
            // a real failure hard to find.
            if let Some(archive) = desired.archive.as_ref() {
                self.apply_archive_schedule(&desired.reference, archive)
                    .await?;
            }

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

    fn take_archive<'a>(
        &'a self,
        reference: &'a IdentityInstanceRef,
        name: &'a str,
    ) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async move {
            let api: Api<IdentityInstanceBackup> =
                Api::namespaced(self.client.clone(), &reference.namespace);

            info!(
                name = %reference.name,
                namespace = %reference.namespace,
                archive = %name,
                "asking for an archive"
            );

            // Server-side applied under the stable field manager, like
            // everything else here. Redelivery of the same action re-asserts
            // the same resource rather than creating a second one, which is
            // what makes the name matter: two names would be two archives of
            // the same database for one request.
            api.patch(
                name,
                &PatchParams::apply(FIELD_MANAGER).force(),
                &Patch::Apply(&to_archive(reference, name)),
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

/// The recurring archive, as the operator reconciles it.
///
/// Pure, so what genesis asks for can be asserted without a cluster.
/// One archive of an instance, named by the caller.
fn to_archive(reference: &IdentityInstanceRef, name: &str) -> IdentityInstanceBackup {
    IdentityInstanceBackup {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(reference.namespace.clone()),
            ..Default::default()
        },
        spec: IdentityInstanceBackupSpec {
            identity_instance_ref: CrdIdentityInstanceRef {
                name: reference.name.clone(),
            },
            // The only mechanism a data plane can carry out today, as with the
            // schedule above: a field with one possible value reads like a
            // choice the control plane made.
            method: Default::default(),
        },
        status: None,
    }
}

fn to_archive_schedule(
    reference: &IdentityInstanceRef,
    archive: &DesiredArchive,
) -> IdentityInstanceBackupSchedule {
    IdentityInstanceBackupSchedule {
        metadata: ObjectMeta {
            name: Some(reference.name.clone()),
            namespace: Some(reference.namespace.clone()),
            ..Default::default()
        },
        spec: IdentityInstanceBackupScheduleSpec {
            identity_instance_ref: CrdIdentityInstanceRef {
                name: reference.name.clone(),
            },
            schedule: archive.schedule.cron.clone(),
            zone: archive.schedule.zone.clone(),
            // The only mechanism a data plane can carry out today. The control
            // plane deliberately sends no method: a field with one possible
            // value reads like a choice.
            method: Default::default(),
            enabled: archive.schedule.enabled,
        },
        status: None,
    }
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
        // Carried whole from the action. Both paths are the source's, computed
        // by the control plane because it owns the archive layout -- rebuilding
        // either here would be a second implementation of the prefix rule, and
        // one that read from the wrong prefix would restore somebody else's
        // data.
        restore: desired.restore.as_ref().map(|restore| RestoreConfig {
            destination_path: restore.destination_path.clone(),
            server_name: restore.server_name.clone(),
            backup_id: restore.backup_id.clone(),
        }),
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
        // The layout comes from the control plane, which owns it. How to reach
        // the store is this data plane's own business, and is filled in here.
        backup: desired.archive.as_ref().map(|archive| BackupConfig {
            destination_path: archive.destination_path.clone(),
            endpoint_url: store_endpoint(),
            credentials_secret: ARCHIVE_CREDENTIALS_SECRET.to_string(),
            encryption: archive.encryption.clone(),
        }),
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
    use crate::domain::entities::identity_instance::{
        DesiredDatabase, DesiredRestore, IdentityInstanceRef,
    };
    use kube::error::ErrorResponse;

    fn desired(provider: IdentityInstanceProvider) -> DesiredIdentityInstance {
        DesiredIdentityInstance {
            restore: None,
            reference: IdentityInstanceRef {
                name: "deployment-b6a1c2d3-e4f5-4a6b-8c9d-0e1f2a3b4c5d".to_string(),
                namespace: "aether-acme-prod".to_string(),
            },
            organisation_id: "9f8e7d6c-5b4a-3c2d-1e0f-a1b2c3d4e5f6".to_string(),
            provider,
            version: "25.0.0".to_string(),
            hostname: "acme-prod.aether-acme-prod.aether.local".to_string(),
            database: DesiredDatabase::from_reserved(500, 1024, 1),
            archive: None,
        }
    }

    /// The two paths are the source's, and they have to reach the CRD whole.
    /// A recovery whose prefix or server name was rebuilt on the way would
    /// read from somewhere nobody wrote to -- and CloudNativePG does not treat
    /// an empty prefix as an error, it comes up empty.
    #[test]
    fn a_recovery_carries_the_prefix_and_the_server_it_reads() {
        let mut desired = desired(IdentityInstanceProvider::Ferriskey);
        desired.restore = Some(DesiredRestore {
            destination_path: "s3://aether-backups/an-org/the-source".to_string(),
            server_name: "deployment-the-source-db".to_string(),
            backup_id: Some("an-archive".to_string()),
        });

        let restore = to_identity_instance(&desired)
            .spec
            .restore
            .expect("the recovery lost what it reads");

        assert_eq!(
            restore.destination_path,
            "s3://aether-backups/an-org/the-source"
        );
        assert_eq!(restore.server_name, "deployment-the-source-db");
        assert_eq!(restore.backup_id.as_deref(), Some("an-archive"));
    }

    /// An ordinary instance says nothing about restoring, so the operator
    /// bootstraps it empty. Absence is the statement, not an empty object.
    #[test]
    fn an_instance_nobody_restored_carries_no_recovery() {
        assert!(
            to_identity_instance(&desired(IdentityInstanceProvider::Ferriskey))
                .spec
                .restore
                .is_none()
        );
    }

    fn archiving() -> DesiredIdentityInstance {
        DesiredIdentityInstance {
            restore: None,
            archive: Some(DesiredArchive {
                destination_path: "s3://aether-backups/an-org/a-deployment".to_string(),
                encryption: Some("AES256".to_string()),
                schedule: crate::domain::entities::identity_instance::DesiredArchiveSchedule {
                    cron: "0 30 2 * * *".to_string(),
                    zone: "Europe/Paris".to_string(),
                    enabled: true,
                },
            }),
            ..desired(IdentityInstanceProvider::Ferriskey)
        }
    }

    /// The half of the chain that was missing: the operator has known how to
    /// build a `barmanObjectStore` section since the CRDs landed, and the only
    /// producer of an `IdentityInstance` wrote `backup: None`.
    #[test]
    fn an_instance_that_archives_carries_where_to() {
        let backup = to_identity_instance(&archiving())
            .spec
            .backup
            .expect("the instance archives");

        assert_eq!(
            backup.destination_path,
            "s3://aether-backups/an-org/a-deployment"
        );
        assert_eq!(backup.credentials_secret, ARCHIVE_CREDENTIALS_SECRET);
        assert_eq!(backup.encryption.as_deref(), Some("AES256"));
    }

    /// Absent, not empty. An empty `barmanObjectStore` is a destination of "",
    /// which CloudNativePG accepts and then fails on at archive time.
    #[test]
    fn an_instance_that_archives_nowhere_says_so_by_absence() {
        assert!(
            to_identity_instance(&desired(IdentityInstanceProvider::Ferriskey))
                .spec
                .backup
                .is_none()
        );
    }

    #[test]
    fn the_schedule_is_named_after_the_instance_it_belongs_to() {
        let instance = archiving();
        let schedule = to_archive_schedule(&instance.reference, instance.archive.as_ref().unwrap());

        // Named after the instance rather than generated: a second name would
        // be a second answer to when this is backed up, and every apply would
        // leave the previous one behind.
        assert_eq!(
            schedule.metadata.name.as_deref(),
            Some(instance.reference.name.as_str())
        );
        assert_eq!(
            schedule.metadata.namespace.as_deref(),
            Some(instance.reference.namespace.as_str())
        );
        assert_eq!(
            schedule.spec.identity_instance_ref.name,
            instance.reference.name
        );
    }

    /// Six fields, seconds first, and no `CRON_TZ` prefix: CloudNativePG's
    /// webhook counts whitespace separated fields and refuses seven.
    #[test]
    fn the_schedule_travels_local_with_its_zone_beside_it() {
        let instance = archiving();
        let schedule = to_archive_schedule(&instance.reference, instance.archive.as_ref().unwrap());

        assert_eq!(schedule.spec.schedule, "0 30 2 * * *");
        assert_eq!(schedule.spec.schedule.split_whitespace().count(), 6);
        assert_eq!(schedule.spec.zone, "Europe/Paris");
        assert!(schedule.spec.enabled);
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
