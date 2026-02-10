use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use aether_crds::v1alpha::identity_instance::{IdentityInstance, IdentityProvider};
use futures::StreamExt;
use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec};
use k8s_openapi::api::core::v1::{
    Container, ContainerPort, EnvVar, EnvVarSource, HTTPGetAction, PodSpec, PodTemplateSpec, Probe,
    Secret, SecretKeySelector, Service, ServicePort, ServiceSpec,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta, OwnerReference};
use kube::core::{ApiResource, DynamicObject, GroupVersionKind};
use kube::runtime::controller::{Action, Controller};
use kube::runtime::events::{Event as KubeEvent, EventType, Recorder, Reporter};
use kube::runtime::watcher;
use kube::{Api, Client, Resource};
use rand::{Rng, distributions::Alphanumeric};
use serde_json::json;
use tracing::{error, info, warn};

use crate::application::OperatorApplication;
use crate::domain::ports::{
    IdentityInstanceDeployer, IdentityInstanceRepository, IdentityInstanceService,
};
use crate::domain::{OperatorError, ReconcileOutcome};

pub struct KubeIdentityInstanceRepository {
    client: Client,
}

impl KubeIdentityInstanceRepository {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl IdentityInstanceRepository for KubeIdentityInstanceRepository {
    async fn patch_status(
        &self,
        instance: &IdentityInstance,
        status: aether_crds::v1alpha::identity_instance::IdentityInstanceStatus,
    ) -> Result<IdentityInstance, OperatorError> {
        let previous_status = instance.status.clone().unwrap_or_default();
        let name = instance
            .metadata
            .name
            .clone()
            .ok_or(OperatorError::MissingName)?;

        let namespace = instance
            .metadata
            .namespace
            .clone()
            .ok_or_else(|| OperatorError::MissingNamespace { name: name.clone() })?;

        let api: Api<IdentityInstance> = Api::namespaced(self.client.clone(), &namespace);
        let patch = json!({ "status": status });

        let updated = api
            .patch_status(
                &name,
                &kube::api::PatchParams::default(),
                &kube::api::Patch::Merge(&patch),
            )
            .await
            .map_err(|error| OperatorError::Kube {
                message: error.to_string(),
            })?;

        if let Err(error) = self
            .publish_status_event(instance, &previous_status, &status)
            .await
        {
            warn!(
                name = %name,
                namespace = %namespace,
                error = %error,
                "Failed to publish IdentityInstance status event"
            );
        }

        Ok(updated)
    }
}

impl KubeIdentityInstanceRepository {
    async fn publish_status_event(
        &self,
        instance: &IdentityInstance,
        previous: &aether_crds::v1alpha::identity_instance::IdentityInstanceStatus,
        current: &aether_crds::v1alpha::identity_instance::IdentityInstanceStatus,
    ) -> Result<(), OperatorError> {
        let previous_phase = previous
            .phase
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "None".to_string());
        let current_phase = current
            .phase
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "None".to_string());

        let reporter = Reporter {
            controller: "aether-operator".to_string(),
            instance: Some("identityinstance-controller".to_string()),
        };
        let recorder = Recorder::new(self.client.clone(), reporter);
        let reference = instance.object_ref(&());
        let phase_note = match current_phase.as_str() {
            "DatabaseProvisioning" => "Database cluster provisioning is in progress.",
            "Deploying" => "Database is ready. Deploying identity provider resources.",
            "Running" => "Identity provider is healthy and ready.",
            _ => "IdentityInstance status updated.",
        };

        let event = KubeEvent {
            type_: EventType::Normal,
            reason: "StatusUpdated".to_string(),
            note: Some(format!(
                "{phase_note} Transition: phase {previous_phase} -> {current_phase}, ready {} -> {}",
                previous.ready, current.ready,
            )),
            action: "StatusPatch".to_string(),
            secondary: None,
        };

        recorder
            .publish(&event, &reference)
            .await
            .map_err(|error| OperatorError::Kube {
                message: error.to_string(),
            })?;

        Ok(())
    }
}

pub struct KubeIdentityInstanceDeployer {
    handlers: Vec<Arc<dyn IdentityProviderHandler>>,
}

impl KubeIdentityInstanceDeployer {
    pub fn new(client: Client) -> Self {
        let handlers: Vec<Arc<dyn IdentityProviderHandler>> = vec![
            Arc::new(KeycloakProviderHandler::new(client)),
            Arc::new(FerriskeyProviderHandler),
        ];
        Self { handlers }
    }

    fn handler_for(
        &self,
        provider: &IdentityProvider,
    ) -> Option<&Arc<dyn IdentityProviderHandler>> {
        self.handlers
            .iter()
            .find(|handler| handler.provider() == *provider)
    }
}

impl IdentityInstanceDeployer for KubeIdentityInstanceDeployer {
    async fn ensure_provider_resources(
        &self,
        instance: &IdentityInstance,
    ) -> Result<(), OperatorError> {
        let provider = &instance.spec.provider;
        let handler = self
            .handler_for(provider)
            .ok_or_else(|| OperatorError::Internal {
                message: format!("no deployer handler registered for provider `{provider}`"),
            })?;
        handler.ensure(instance).await
    }

    async fn cleanup_provider_resources(
        &self,
        instance: &IdentityInstance,
    ) -> Result<(), OperatorError> {
        let provider = &instance.spec.provider;
        let handler = self
            .handler_for(provider)
            .ok_or_else(|| OperatorError::Internal {
                message: format!("no deployer handler registered for provider `{provider}`"),
            })?;
        handler.cleanup(instance).await
    }

    async fn provider_ready(&self, instance: &IdentityInstance) -> Result<bool, OperatorError> {
        let provider = &instance.spec.provider;
        let handler = self
            .handler_for(provider)
            .ok_or_else(|| OperatorError::Internal {
                message: format!("no deployer handler registered for provider `{provider}`"),
            })?;
        handler.ready(instance).await
    }

    async fn database_ready(&self, instance: &IdentityInstance) -> Result<bool, OperatorError> {
        let provider = &instance.spec.provider;
        let handler = self
            .handler_for(provider)
            .ok_or_else(|| OperatorError::Internal {
                message: format!("no deployer handler registered for provider `{provider}`"),
            })?;
        handler.database_ready(instance).await
    }
}

type ProviderFuture<'a> = Pin<Box<dyn Future<Output = Result<(), OperatorError>> + Send + 'a>>;
type ProviderReadyFuture<'a> =
    Pin<Box<dyn Future<Output = Result<bool, OperatorError>> + Send + 'a>>;

trait IdentityProviderHandler: Send + Sync {
    fn provider(&self) -> IdentityProvider;
    fn ensure<'a>(&'a self, instance: &'a IdentityInstance) -> ProviderFuture<'a>;
    fn cleanup<'a>(&'a self, instance: &'a IdentityInstance) -> ProviderFuture<'a>;
    fn ready<'a>(&'a self, instance: &'a IdentityInstance) -> ProviderReadyFuture<'a>;
    fn database_ready<'a>(&'a self, instance: &'a IdentityInstance) -> ProviderReadyFuture<'a>;
}

struct KeycloakProviderHandler {
    client: Client,
}

impl KeycloakProviderHandler {
    fn new(client: Client) -> Self {
        Self { client }
    }

    async fn ensure_keycloak_admin_secret(
        &self,
        namespace: &str,
        secret_name: &str,
        owner_reference: Option<OwnerReference>,
    ) -> Result<(), OperatorError> {
        let secrets: Api<Secret> = Api::namespaced(self.client.clone(), namespace);

        if let Some(existing) =
            secrets
                .get_opt(secret_name)
                .await
                .map_err(|error| OperatorError::Kube {
                    message: error.to_string(),
                })?
            && let Some(data) = existing.data.as_ref()
            && data.contains_key("username")
            && data.contains_key("password")
        {
            return Ok(());
        }

        let password = generate_password(32);
        let mut string_data = BTreeMap::new();
        string_data.insert("username".to_string(), "admin".to_string());
        string_data.insert("password".to_string(), password);

        let secret = Secret {
            metadata: ObjectMeta {
                name: Some(secret_name.to_string()),
                namespace: Some(namespace.to_string()),
                owner_references: owner_reference.map(|owner| vec![owner]),
                ..Default::default()
            },
            type_: Some("Opaque".to_string()),
            string_data: Some(string_data),
            ..Default::default()
        };

        secrets
            .patch(
                secret_name,
                &kube::api::PatchParams::apply("aether-operator").force(),
                &kube::api::Patch::Apply(&secret),
            )
            .await
            .map_err(|error| OperatorError::Kube {
                message: error.to_string(),
            })?;

        Ok(())
    }

    async fn ensure_keycloak_db_credentials_secret(
        &self,
        instance: &IdentityInstance,
        namespace: &str,
        owner_reference: Option<OwnerReference>,
    ) -> Result<bool, OperatorError> {
        let instance_name = instance
            .metadata
            .name
            .clone()
            .ok_or(OperatorError::MissingName)?;
        let target_secret_name = keycloak_db_credentials_secret_name(&instance_name);
        let source_secret_name = format!("{}-app", cnpg_cluster_name(instance));
        let secrets: Api<Secret> = Api::namespaced(self.client.clone(), namespace);

        if let Some(existing) = secrets
            .get_opt(&target_secret_name)
            .await
            .map_err(|error| OperatorError::Kube {
                message: error.to_string(),
            })?
            && let Some(data) = existing.data.as_ref()
            && data.contains_key("jdbc-uri")
            && data.contains_key("user")
            && data.contains_key("password")
        {
            return Ok(true);
        }

        let source = secrets
            .get_opt(&source_secret_name)
            .await
            .map_err(|error| OperatorError::Kube {
                message: error.to_string(),
            })?;
        let Some(source) = source else {
            return Ok(false);
        };

        let data = source.data.ok_or_else(|| OperatorError::Internal {
            message: format!("CNPG secret `{}` has no data", source_secret_name),
        })?;

        let username = secret_data_value(&data, "username")
            .or_else(|| secret_data_value(&data, "user"))
            .ok_or_else(|| OperatorError::Internal {
                message: format!("CNPG secret `{}` missing `username`", source_secret_name),
            })?;
        let password =
            secret_data_value(&data, "password").ok_or_else(|| OperatorError::Internal {
                message: format!("CNPG secret `{}` missing `password`", source_secret_name),
            })?;
        let jdbc_uri = secret_data_value(&data, "jdbc-uri")
            .or_else(|| {
                // Fallback for CNPG variants that provide only `uri`.
                secret_data_value(&data, "uri").map(|uri| {
                    if uri.starts_with("jdbc:") {
                        uri
                    } else if let Some(rest) = uri.strip_prefix("postgresql://") {
                        format!("jdbc:postgresql://{rest}")
                    } else if let Some(rest) = uri.strip_prefix("postgres://") {
                        format!("jdbc:postgresql://{rest}")
                    } else {
                        uri
                    }
                })
            })
            .ok_or_else(|| OperatorError::Internal {
                message: format!(
                    "CNPG secret `{}` missing `jdbc-uri` and `uri`",
                    source_secret_name
                ),
            })?;

        let mut string_data = BTreeMap::new();
        string_data.insert("user".to_string(), username);
        string_data.insert("password".to_string(), password);
        string_data.insert("jdbc-uri".to_string(), jdbc_uri);

        let secret = Secret {
            metadata: ObjectMeta {
                name: Some(target_secret_name.clone()),
                namespace: Some(namespace.to_string()),
                owner_references: owner_reference.map(|owner| vec![owner]),
                ..Default::default()
            },
            type_: Some("Opaque".to_string()),
            string_data: Some(string_data),
            ..Default::default()
        };

        secrets
            .patch(
                &target_secret_name,
                &kube::api::PatchParams::apply("aether-operator").force(),
                &kube::api::Patch::Apply(&secret),
            )
            .await
            .map_err(|error| OperatorError::Kube {
                message: error.to_string(),
            })?;

        Ok(true)
    }

    async fn keycloak_ready(&self, instance: &IdentityInstance) -> Result<bool, OperatorError> {
        let name = instance
            .metadata
            .name
            .clone()
            .ok_or(OperatorError::MissingName)?;
        let namespace = instance
            .metadata
            .namespace
            .clone()
            .ok_or_else(|| OperatorError::MissingNamespace { name: name.clone() })?;
        let deployments: Api<Deployment> = Api::namespaced(self.client.clone(), &namespace);

        let deployment = deployments
            .get_opt(&name)
            .await
            .map_err(|error| OperatorError::Kube {
                message: error.to_string(),
            })?;

        let Some(deployment) = deployment else {
            return Ok(false);
        };

        // IdentityInstance currently drives a single replica deployment.
        let desired_replicas = 1;
        let generation = deployment.metadata.generation.unwrap_or_default();
        let observed_generation = deployment
            .status
            .as_ref()
            .and_then(|status| status.observed_generation)
            .unwrap_or_default();
        let ready_replicas = deployment
            .status
            .as_ref()
            .and_then(|status| status.ready_replicas)
            .unwrap_or(0);
        let available_replicas = deployment
            .status
            .as_ref()
            .and_then(|status| status.available_replicas)
            .unwrap_or(0);
        // `updated_replicas` can stay unset depending on rollout history; rely on
        // observed generation + ready/available replicas to decide readiness.
        Ok(observed_generation >= generation
            && ready_replicas >= desired_replicas
            && available_replicas >= desired_replicas)
    }

    async fn ensure_managed_db_cluster(
        &self,
        instance: &IdentityInstance,
        namespace: &str,
        owner_reference: Option<OwnerReference>,
    ) -> Result<(), OperatorError> {
        let cluster_name = cnpg_cluster_name(instance);
        let gvk = GroupVersionKind::gvk("postgresql.cnpg.io", "v1", "Cluster");
        let ar = ApiResource::from_gvk(&gvk);
        let clusters: Api<DynamicObject> =
            Api::namespaced_with(self.client.clone(), namespace, &ar);

        let managed_cluster = &instance.spec.database.managed_cluster;
        let mut spec = serde_json::Map::new();
        spec.insert("instances".to_string(), json!(managed_cluster.instances));
        let mut storage = serde_json::Map::new();
        storage.insert("size".to_string(), json!(managed_cluster.storage.size));
        if let Some(storage_class) = managed_cluster.storage.storage_class.as_ref() {
            storage.insert("storageClass".to_string(), json!(storage_class));
        }
        spec.insert("storage".to_string(), serde_json::Value::Object(storage));

        if let Some(resources) = cnpg_resources_json(&managed_cluster.resources) {
            spec.insert("resources".to_string(), resources);
        }

        let cluster_manifest = json!({
            "apiVersion": "postgresql.cnpg.io/v1",
            "kind": "Cluster",
            "metadata": {
                "name": cluster_name,
                "namespace": namespace,
                "ownerReferences": owner_reference.map(|owner| vec![owner]),
            },
            "spec": spec
        });

        clusters
            .patch(
                &cluster_name,
                &kube::api::PatchParams::apply("aether-operator").force(),
                &kube::api::Patch::Apply(&cluster_manifest),
            )
            .await
            .map_err(|error| OperatorError::Kube {
                message: error.to_string(),
            })?;

        Ok(())
    }

    async fn cnpg_cluster_ready(&self, instance: &IdentityInstance) -> Result<bool, OperatorError> {
        let namespace = instance
            .metadata
            .namespace
            .clone()
            .ok_or_else(|| OperatorError::MissingNamespace {
                name: instance.metadata.name.clone().unwrap_or_default(),
            })?;
        let cluster_name = cnpg_cluster_name(instance);
        let gvk = GroupVersionKind::gvk("postgresql.cnpg.io", "v1", "Cluster");
        let ar = ApiResource::from_gvk(&gvk);
        let clusters: Api<DynamicObject> =
            Api::namespaced_with(self.client.clone(), &namespace, &ar);

        let cluster =
            clusters
                .get_opt(&cluster_name)
                .await
                .map_err(|error| OperatorError::Kube {
                    message: error.to_string(),
                })?;

        let Some(cluster) = cluster else {
            return Ok(false);
        };

        let status = cluster.data.get("status");
        let Some(conditions) = status
            .and_then(|status| status.get("conditions"))
            .and_then(|conditions| conditions.as_array())
        else {
            return Ok(false);
        };

        let ready = conditions.iter().any(|condition| {
            condition
                .get("type")
                .and_then(|value| value.as_str())
                == Some("Ready")
                && condition
                    .get("status")
                    .and_then(|value| value.as_str())
                    == Some("True")
        });

        Ok(ready)
    }
}

impl IdentityProviderHandler for KeycloakProviderHandler {
    fn provider(&self) -> IdentityProvider {
        IdentityProvider::Keycloak
    }

    fn ensure<'a>(&'a self, instance: &'a IdentityInstance) -> ProviderFuture<'a> {
        Box::pin(async move {
            let name = instance
                .metadata
                .name
                .clone()
                .ok_or(OperatorError::MissingName)?;
            let namespace = instance
                .metadata
                .namespace
                .clone()
                .ok_or_else(|| OperatorError::MissingNamespace { name: name.clone() })?;

            info!(
                name = %name,
                namespace = %namespace,
                provider = "keycloak",
                "Ensuring provider resources"
            );

            let admin_secret_name = keycloak_admin_secret_name(&name);
            let owner_reference = instance.controller_owner_ref(&());
            self.ensure_managed_db_cluster(instance, &namespace, owner_reference.clone())
                .await?;
            let db_cluster_ready = self.cnpg_cluster_ready(instance).await?;
            if !db_cluster_ready {
                info!(
                    name = %name,
                    namespace = %namespace,
                    provider = "keycloak",
                    "Waiting for CNPG cluster readiness before deploying provider resources"
                );
                return Ok(());
            }
            let db_secret_ready = self
                .ensure_keycloak_db_credentials_secret(
                instance,
                &namespace,
                owner_reference.clone(),
            )
            .await?;
            if !db_secret_ready {
                info!(
                    name = %name,
                    namespace = %namespace,
                    provider = "keycloak",
                    "Waiting for CNPG credentials secret before deploying provider resources"
                );
                return Ok(());
            }
            self.ensure_keycloak_admin_secret(
                &namespace,
                &admin_secret_name,
                owner_reference.clone(),
            )
            .await?;

            let labels = keycloak_labels(instance);
            let deployment = build_keycloak_deployment(
                instance,
                &name,
                &namespace,
                &labels,
                &admin_secret_name,
                owner_reference.clone(),
            )?;
            let service =
                build_keycloak_service(instance, &name, &namespace, &labels, owner_reference)?;

            let deployments: Api<Deployment> = Api::namespaced(self.client.clone(), &namespace);
            let services: Api<Service> = Api::namespaced(self.client.clone(), &namespace);

            let params = kube::api::PatchParams::apply("aether-operator").force();
            deployments
                .patch(&name, &params, &kube::api::Patch::Apply(&deployment))
                .await
                .map_err(|error| OperatorError::Kube {
                    message: error.to_string(),
                })?;
            services
                .patch(&name, &params, &kube::api::Patch::Apply(&service))
                .await
                .map_err(|error| OperatorError::Kube {
                    message: error.to_string(),
                })?;

            info!(
                name = %name,
                namespace = %namespace,
                provider = "keycloak",
                "Provider resources applied"
            );

            Ok(())
        })
    }

    fn cleanup<'a>(&'a self, instance: &'a IdentityInstance) -> ProviderFuture<'a> {
        Box::pin(async move {
            let name = instance
                .metadata
                .name
                .clone()
                .ok_or(OperatorError::MissingName)?;
            let namespace = instance
                .metadata
                .namespace
                .clone()
                .ok_or_else(|| OperatorError::MissingNamespace { name: name.clone() })?;
            let delete_params = kube::api::DeleteParams::default();

            info!(
                name = %name,
                namespace = %namespace,
                provider = "keycloak",
                "Cleaning up provider resources"
            );

            let deployments: Api<Deployment> = Api::namespaced(self.client.clone(), &namespace);
            if let Err(error) = deployments.delete(&name, &delete_params).await
                && !is_not_found(&error)
            {
                return Err(OperatorError::Kube {
                    message: error.to_string(),
                });
            }

            let services: Api<Service> = Api::namespaced(self.client.clone(), &namespace);
            if let Err(error) = services.delete(&name, &delete_params).await
                && !is_not_found(&error)
            {
                return Err(OperatorError::Kube {
                    message: error.to_string(),
                });
            }

            let secrets: Api<Secret> = Api::namespaced(self.client.clone(), &namespace);
            let admin_secret = keycloak_admin_secret_name(&name);
            if let Err(error) = secrets.delete(&admin_secret, &delete_params).await
                && !is_not_found(&error)
            {
                return Err(OperatorError::Kube {
                    message: error.to_string(),
                });
            }

            Ok(())
        })
    }

    fn ready<'a>(&'a self, instance: &'a IdentityInstance) -> ProviderReadyFuture<'a> {
        Box::pin(async move { self.keycloak_ready(instance).await })
    }

    fn database_ready<'a>(&'a self, instance: &'a IdentityInstance) -> ProviderReadyFuture<'a> {
        Box::pin(async move { self.cnpg_cluster_ready(instance).await })
    }
}

struct FerriskeyProviderHandler;

impl IdentityProviderHandler for FerriskeyProviderHandler {
    fn provider(&self) -> IdentityProvider {
        IdentityProvider::Ferriskey
    }

    fn ensure<'a>(&'a self, instance: &'a IdentityInstance) -> ProviderFuture<'a> {
        Box::pin(async move {
            let name = instance.metadata.name.clone().unwrap_or_default();
            let namespace = instance.metadata.namespace.clone().unwrap_or_default();
            warn!(
                name = %name,
                namespace = %namespace,
                provider = "ferriskey",
                "Provider handler is registered but resource reconciliation is not implemented yet"
            );
            Ok(())
        })
    }

    fn cleanup<'a>(&'a self, instance: &'a IdentityInstance) -> ProviderFuture<'a> {
        Box::pin(async move {
            let name = instance.metadata.name.clone().unwrap_or_default();
            let namespace = instance.metadata.namespace.clone().unwrap_or_default();
            warn!(
                name = %name,
                namespace = %namespace,
                provider = "ferriskey",
                "Provider handler is registered but cleanup is not implemented yet"
            );
            Ok(())
        })
    }

    fn ready<'a>(&'a self, _instance: &'a IdentityInstance) -> ProviderReadyFuture<'a> {
        Box::pin(async move { Ok(false) })
    }

    fn database_ready<'a>(&'a self, _instance: &'a IdentityInstance) -> ProviderReadyFuture<'a> {
        Box::pin(async move { Ok(false) })
    }
}

#[derive(Clone)]
struct OperatorContext<S, D> {
    service: Arc<S>,
    deployer: Arc<D>,
    client: Client,
}

async fn reconcile<S, D>(
    instance: Arc<IdentityInstance>,
    context: Arc<OperatorContext<S, D>>,
) -> Result<Action, OperatorError>
where
    S: IdentityInstanceService,
    D: IdentityInstanceDeployer,
{
    if instance.metadata.deletion_timestamp.is_some() {
        return handle_deletion(instance, context).await;
    }

    ensure_finalizer(&instance, &context.client).await?;
    let name = instance.metadata.name.clone().unwrap_or_default();
    let namespace = instance.metadata.namespace.clone().unwrap_or_default();
    info!(
        name = %name,
        namespace = %namespace,
        "Reconciling IdentityInstance"
    );

    let outcome = context.service.reconcile((*instance).clone()).await?;
    Ok(outcome_to_action(outcome))
}

fn error_policy<S, D>(
    _instance: Arc<IdentityInstance>,
    error: &OperatorError,
    _context: Arc<OperatorContext<S, D>>,
) -> Action {
    error!(error = %error, "Reconcile error");
    error_requeue_action()
}

fn error_requeue_action() -> Action {
    Action::requeue(Duration::from_secs(30))
}

fn outcome_to_action(outcome: ReconcileOutcome) -> Action {
    match outcome.requeue_after {
        Some(delay) => Action::requeue(delay),
        None => Action::await_change(),
    }
}

pub async fn run() -> Result<(), OperatorError> {
    info!("Starting Aether operator");
    let client = Client::try_default()
        .await
        .map_err(|error| OperatorError::Kube {
            message: error.to_string(),
        })?;
    let repository = Arc::new(KubeIdentityInstanceRepository::new(client.clone()));
    let deployer = Arc::new(KubeIdentityInstanceDeployer::new(client.clone()));
    let service = Arc::new(OperatorApplication::new(repository, deployer.clone()));

    let instances = Api::<IdentityInstance>::all(client.clone());
    let deployments = Api::<Deployment>::all(client.clone());
    let context = Arc::new(OperatorContext {
        service,
        deployer,
        client: client.clone(),
    });

    Controller::new(instances, watcher::Config::default())
        .owns(deployments, watcher::Config::default())
        .run(
            reconcile::<
                OperatorApplication<KubeIdentityInstanceRepository, KubeIdentityInstanceDeployer>,
                KubeIdentityInstanceDeployer,
            >,
            error_policy::<
                OperatorApplication<KubeIdentityInstanceRepository, KubeIdentityInstanceDeployer>,
                KubeIdentityInstanceDeployer,
            >,
            context,
        )
        .for_each(|_| async {})
        .await;

    Ok(())
}

const FINALIZER_NAME: &str = "aether.dev/identityinstance-cleanup";

async fn ensure_finalizer(
    instance: &IdentityInstance,
    client: &Client,
) -> Result<(), OperatorError> {
    let name = instance
        .metadata
        .name
        .clone()
        .ok_or(OperatorError::MissingName)?;
    let namespace = instance
        .metadata
        .namespace
        .clone()
        .ok_or_else(|| OperatorError::MissingNamespace { name: name.clone() })?;
    let mut finalizers = instance.metadata.finalizers.clone().unwrap_or_default();
    if finalizers.iter().any(|item| item == FINALIZER_NAME) {
        return Ok(());
    }

    finalizers.push(FINALIZER_NAME.to_string());
    let api: Api<IdentityInstance> = Api::namespaced(client.clone(), &namespace);
    let patch = json!({ "metadata": { "finalizers": finalizers } });
    api.patch(
        &name,
        &kube::api::PatchParams::default(),
        &kube::api::Patch::Merge(&patch),
    )
    .await
    .map_err(|error| OperatorError::Kube {
        message: error.to_string(),
    })?;

    Ok(())
}

async fn handle_deletion<S, D>(
    instance: Arc<IdentityInstance>,
    context: Arc<OperatorContext<S, D>>,
) -> Result<Action, OperatorError>
where
    S: IdentityInstanceService,
    D: IdentityInstanceDeployer,
{
    let name = instance.metadata.name.clone().unwrap_or_default();
    let namespace = instance.metadata.namespace.clone().unwrap_or_default();

    context
        .deployer
        .cleanup_provider_resources(&instance)
        .await?;

    let mut finalizers = instance.metadata.finalizers.clone().unwrap_or_default();
    finalizers.retain(|item| item != FINALIZER_NAME);
    let api: Api<IdentityInstance> = Api::namespaced(context.client.clone(), &namespace);
    let patch = json!({ "metadata": { "finalizers": finalizers } });
    api.patch(
        &name,
        &kube::api::PatchParams::default(),
        &kube::api::Patch::Merge(&patch),
    )
    .await
    .map_err(|error| OperatorError::Kube {
        message: error.to_string(),
    })?;

    info!(
        name = %name,
        namespace = %namespace,
        "Cleanup completed, finalizer removed"
    );

    Ok(Action::await_change())
}

fn is_not_found(error: &kube::Error) -> bool {
    matches!(error, kube::Error::Api(api_error) if api_error.code == 404)
}

fn keycloak_labels(instance: &IdentityInstance) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::new();
    labels.insert("app.kubernetes.io/name".to_string(), "keycloak".to_string());
    labels.insert(
        "app.kubernetes.io/instance".to_string(),
        instance.metadata.name.clone().unwrap_or_default(),
    );
    labels
}

fn build_keycloak_deployment(
    instance: &IdentityInstance,
    name: &str,
    namespace: &str,
    labels: &BTreeMap<String, String>,
    admin_secret_name: &str,
    owner_reference: Option<OwnerReference>,
) -> Result<Deployment, OperatorError> {
    let image = format!("quay.io/keycloak/keycloak:{}", instance.spec.version);
    let credentials_secret = keycloak_db_credentials_secret_name(name);
    let selector = LabelSelector {
        match_labels: Some(labels.clone()),
        ..Default::default()
    };

    Ok(Deployment {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            labels: Some(labels.clone()),
            owner_references: owner_reference.map(|owner| vec![owner]),
            ..Default::default()
        },
        spec: Some(DeploymentSpec {
            replicas: Some(1),
            selector,
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(labels.clone()),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    containers: vec![Container {
                        name: "keycloak".to_string(),
                        image: Some(image),
                        args: Some(vec![
                            "start-dev".to_string(),
                            "--health-enabled=true".to_string(),
                        ]),
                        ports: Some(vec![
                            ContainerPort {
                                container_port: 8080,
                                ..Default::default()
                            },
                            ContainerPort {
                                container_port: 9000,
                                ..Default::default()
                            },
                        ]),
                        env: Some(vec![
                            EnvVar {
                                name: "KC_DB".to_string(),
                                value: Some("postgres".to_string()),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "KC_DB_URL".to_string(),
                                value_from: Some(EnvVarSource {
                                    secret_key_ref: Some(SecretKeySelector {
                                        name: credentials_secret.clone(),
                                        key: "jdbc-uri".to_string(),
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "KC_DB_USERNAME".to_string(),
                                value_from: Some(EnvVarSource {
                                    secret_key_ref: Some(SecretKeySelector {
                                        name: credentials_secret.clone(),
                                        key: "user".to_string(),
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "KC_DB_PASSWORD".to_string(),
                                value_from: Some(EnvVarSource {
                                    secret_key_ref: Some(SecretKeySelector {
                                        name: credentials_secret.clone(),
                                        key: "password".to_string(),
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "KC_HOSTNAME".to_string(),
                                value: Some(instance.spec.hostname.clone()),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "KEYCLOAK_ADMIN".to_string(),
                                value_from: Some(EnvVarSource {
                                    secret_key_ref: Some(SecretKeySelector {
                                        name: admin_secret_name.to_string(),
                                        key: "username".to_string(),
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                            EnvVar {
                                name: "KEYCLOAK_ADMIN_PASSWORD".to_string(),
                                value_from: Some(EnvVarSource {
                                    secret_key_ref: Some(SecretKeySelector {
                                        name: admin_secret_name.to_string(),
                                        key: "password".to_string(),
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                        ]),
                        startup_probe: Some(Probe {
                            http_get: Some(HTTPGetAction {
                                path: Some("/health/started".to_string()),
                                port:
                                    k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::Int(
                                        9000,
                                    ),
                                scheme: Some("HTTP".to_string()),
                                ..Default::default()
                            }),
                            failure_threshold: Some(60),
                            period_seconds: Some(5),
                            timeout_seconds: Some(2),
                            ..Default::default()
                        }),
                        readiness_probe: Some(Probe {
                            http_get: Some(HTTPGetAction {
                                path: Some("/health/ready".to_string()),
                                port:
                                    k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::Int(
                                        9000,
                                    ),
                                scheme: Some("HTTP".to_string()),
                                ..Default::default()
                            }),
                            period_seconds: Some(10),
                            timeout_seconds: Some(2),
                            failure_threshold: Some(6),
                            success_threshold: Some(1),
                            ..Default::default()
                        }),
                        liveness_probe: Some(Probe {
                            http_get: Some(HTTPGetAction {
                                path: Some("/health/live".to_string()),
                                port:
                                    k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::Int(
                                        9000,
                                    ),
                                scheme: Some("HTTP".to_string()),
                                ..Default::default()
                            }),
                            period_seconds: Some(10),
                            timeout_seconds: Some(2),
                            failure_threshold: Some(6),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
            },
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn build_keycloak_service(
    _instance: &IdentityInstance,
    name: &str,
    namespace: &str,
    labels: &BTreeMap<String, String>,
    owner_reference: Option<OwnerReference>,
) -> Result<Service, OperatorError> {
    Ok(Service {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            labels: Some(labels.clone()),
            owner_references: owner_reference.map(|owner| vec![owner]),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            selector: Some(labels.clone()),
            ports: Some(vec![ServicePort {
                port: 80,
                target_port: Some(
                    k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::Int(8080),
                ),
                ..Default::default()
            }]),
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn keycloak_admin_secret_name(instance_name: &str) -> String {
    format!("{instance_name}-admin")
}

fn cnpg_cluster_name(instance: &IdentityInstance) -> String {
    let instance_name = instance.metadata.name.clone().unwrap_or_default();
    format!("{instance_name}-db")
}

fn keycloak_db_credentials_secret_name(instance_name: &str) -> String {
    format!("{instance_name}-db-credentials")
}

fn cnpg_resources_json(
    resources: &aether_crds::common::types::ResourceRequirements,
) -> Option<serde_json::Value> {
    let mut root = serde_json::Map::new();

    if let Some(requests) = resources.requests.as_ref() {
        let mut req = serde_json::Map::new();
        if let Some(cpu) = requests.cpu.as_ref() {
            req.insert("cpu".to_string(), json!(cpu));
        }
        if let Some(memory) = requests.memory.as_ref() {
            req.insert("memory".to_string(), json!(memory));
        }
        if !req.is_empty() {
            root.insert("requests".to_string(), serde_json::Value::Object(req));
        }
    }

    if let Some(limits) = resources.limits.as_ref() {
        let mut lim = serde_json::Map::new();
        if let Some(cpu) = limits.cpu.as_ref() {
            lim.insert("cpu".to_string(), json!(cpu));
        }
        if let Some(memory) = limits.memory.as_ref() {
            lim.insert("memory".to_string(), json!(memory));
        }
        if !lim.is_empty() {
            root.insert("limits".to_string(), serde_json::Value::Object(lim));
        }
    }

    if root.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(root))
    }
}

fn secret_data_value(
    data: &BTreeMap<String, k8s_openapi::ByteString>,
    key: &str,
) -> Option<String> {
    data.get(key)
        .map(|value| String::from_utf8_lossy(&value.0).to_string())
}

fn generate_password(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_crds::common::types::ResourceRequirements;
    use aether_crds::v1alpha::identity_instance::{
        DatabaseConfig, DatabaseMode, IdentityInstance, IdentityInstanceSpec, IdentityProvider,
        ManagedClusterConfig, ManagedClusterStorage,
    };
    use kube::core::ObjectMeta;
    use kube::error::ErrorResponse;

    fn instance() -> IdentityInstance {
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
            status: None,
        }
    }

    fn env_value<'a>(container: &'a Container, name: &str) -> Option<&'a EnvVar> {
        container
            .env
            .as_ref()
            .and_then(|envs| envs.iter().find(|env| env.name == name))
    }

    #[test]
    fn outcome_to_action_maps_requeue() {
        let outcome = ReconcileOutcome::requeue_after(Duration::from_secs(5));
        let action = outcome_to_action(outcome);
        assert_eq!(action, Action::requeue(Duration::from_secs(5)));

        let action = outcome_to_action(ReconcileOutcome::default());
        assert_eq!(action, Action::await_change());
    }

    #[test]
    fn error_policy_requeues_after_30s() {
        let action = error_requeue_action();
        assert_eq!(action, Action::requeue(Duration::from_secs(30)));
    }

    #[test]
    fn is_not_found_detects_404() {
        let not_found = kube::Error::Api(ErrorResponse {
            status: "Failure".to_string(),
            message: "not found".to_string(),
            reason: "NotFound".to_string(),
            code: 404,
        });
        let other = kube::Error::Api(ErrorResponse {
            status: "Failure".to_string(),
            message: "boom".to_string(),
            reason: "Internal".to_string(),
            code: 500,
        });

        assert!(is_not_found(&not_found));
        assert!(!is_not_found(&other));
    }

    #[test]
    fn keycloak_admin_secret_name_formats() {
        assert_eq!(keycloak_admin_secret_name("instance-1"), "instance-1-admin");
    }

    #[test]
    fn keycloak_db_credentials_secret_name_formats() {
        assert_eq!(
            keycloak_db_credentials_secret_name("instance-1"),
            "instance-1-db-credentials"
        );
    }

    #[test]
    fn generate_password_returns_alphanumeric() {
        let password = generate_password(32);
        assert_eq!(password.len(), 32);
        assert!(password.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn keycloak_labels_include_instance_name() {
        let instance = instance();
        let labels = keycloak_labels(&instance);

        assert_eq!(
            labels.get("app.kubernetes.io/name").map(String::as_str),
            Some("keycloak")
        );
        assert_eq!(
            labels.get("app.kubernetes.io/instance").map(String::as_str),
            Some("instance-1")
        );
    }

    #[test]
    fn build_keycloak_service_sets_ports_and_labels() {
        let instance = instance();
        let labels = keycloak_labels(&instance);
        let service =
            build_keycloak_service(&instance, "instance-1", "default", &labels, None).unwrap();

        let metadata = service.metadata;
        assert_eq!(metadata.name.as_deref(), Some("instance-1"));
        assert_eq!(metadata.namespace.as_deref(), Some("default"));
        assert_eq!(metadata.labels, Some(labels.clone()));

        let spec = service.spec.expect("service spec");
        assert_eq!(spec.selector, Some(labels));
        let ports = spec.ports.expect("service ports");
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 80);
    }

    #[test]
    fn build_keycloak_deployment_sets_env_and_image() {
        let instance = instance();
        let labels = keycloak_labels(&instance);
        let deployment = build_keycloak_deployment(
            &instance,
            "instance-1",
            "default",
            &labels,
            "instance-1-admin",
            None,
        )
        .unwrap();

        let metadata = deployment.metadata;
        assert_eq!(metadata.name.as_deref(), Some("instance-1"));
        assert_eq!(metadata.namespace.as_deref(), Some("default"));
        assert_eq!(metadata.labels, Some(labels.clone()));

        let spec = deployment.spec.expect("deployment spec");
        let template = spec.template;
        let pod_spec = template.spec.expect("pod spec");
        let container = &pod_spec.containers[0];

        assert_eq!(container.name, "keycloak");
        assert_eq!(
            container.image.as_deref(),
            Some("quay.io/keycloak/keycloak:25.0.0")
        );
        assert_eq!(
            container.args.as_ref(),
            Some(&vec![
                "start-dev".to_string(),
                "--health-enabled=true".to_string(),
            ])
        );
        assert!(
            container
                .ports
                .as_ref()
                .map(|ports| ports.iter().any(|port| port.container_port == 9000))
                .unwrap_or(false)
        );

        let kc_db_url = env_value(container, "KC_DB_URL").and_then(|env| env.value_from.as_ref());
        let kc_db_user =
            env_value(container, "KC_DB_USERNAME").and_then(|env| env.value_from.as_ref());
        let kc_db_pass =
            env_value(container, "KC_DB_PASSWORD").and_then(|env| env.value_from.as_ref());
        let kc_host = env_value(container, "KC_HOSTNAME").and_then(|env| env.value.as_deref());
        let admin_user =
            env_value(container, "KEYCLOAK_ADMIN").and_then(|env| env.value_from.as_ref());
        let admin_pass =
            env_value(container, "KEYCLOAK_ADMIN_PASSWORD").and_then(|env| env.value_from.as_ref());

        assert_eq!(kc_host, Some("auth.acme.test"));
        assert!(
            kc_db_url
                .and_then(|source| source.secret_key_ref.as_ref())
                .map(|secret| secret.name == "instance-1-db-credentials" && secret.key == "jdbc-uri")
                .unwrap_or(false)
        );
        assert!(
            kc_db_user
                .and_then(|source| source.secret_key_ref.as_ref())
                .map(|secret| secret.name == "instance-1-db-credentials" && secret.key == "user")
                .unwrap_or(false)
        );
        assert!(
            kc_db_pass
                .and_then(|source| source.secret_key_ref.as_ref())
                .map(|secret| secret.name == "instance-1-db-credentials" && secret.key == "password")
                .unwrap_or(false)
        );
        assert!(
            admin_user
                .and_then(|source| source.secret_key_ref.as_ref())
                .map(|secret| secret.name == "instance-1-admin" && secret.key == "username")
                .unwrap_or(false)
        );
        assert!(
            admin_pass
                .and_then(|source| source.secret_key_ref.as_ref())
                .map(|secret| secret.name == "instance-1-admin" && secret.key == "password")
                .unwrap_or(false)
        );

        let startup_probe = container
            .startup_probe
            .as_ref()
            .and_then(|probe| probe.http_get.as_ref());
        let readiness_probe = container
            .readiness_probe
            .as_ref()
            .and_then(|probe| probe.http_get.as_ref());
        let liveness_probe = container
            .liveness_probe
            .as_ref()
            .and_then(|probe| probe.http_get.as_ref());

        assert_eq!(
            startup_probe.and_then(|get| get.path.as_deref()),
            Some("/health/started")
        );
        assert_eq!(
            startup_probe.map(|get| get.port.clone()),
            Some(k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::Int(9000))
        );
        assert_eq!(
            readiness_probe.and_then(|get| get.path.as_deref()),
            Some("/health/ready")
        );
        assert_eq!(
            readiness_probe.map(|get| get.port.clone()),
            Some(k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::Int(9000))
        );
        assert_eq!(
            liveness_probe.and_then(|get| get.path.as_deref()),
            Some("/health/live")
        );
        assert_eq!(
            liveness_probe.map(|get| get.port.clone()),
            Some(k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::Int(9000))
        );
    }
}
