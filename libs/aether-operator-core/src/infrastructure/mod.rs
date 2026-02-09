use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use aether_crds::v1alpha::identity_instance::IdentityInstance;
use futures::StreamExt;
use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec};
use k8s_openapi::api::core::v1::{
    Container, ContainerPort, EnvVar, EnvVarSource, PodSpec, PodTemplateSpec, Secret,
    SecretKeySelector, Service, ServicePort, ServiceSpec,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};
use kube::runtime::controller::{Action, Controller};
use kube::runtime::watcher;
use kube::{Api, Client};
use rand::{Rng, distributions::Alphanumeric};
use serde_json::json;
use tracing::{error, info};

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

        api.patch_status(
            &name,
            &kube::api::PatchParams::default(),
            &kube::api::Patch::Merge(&patch),
        )
        .await
        .map_err(|error| OperatorError::Kube {
            message: error.to_string(),
        })
    }
}

pub struct KubeIdentityInstanceDeployer {
    client: Client,
}

impl KubeIdentityInstanceDeployer {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl IdentityInstanceDeployer for KubeIdentityInstanceDeployer {
    async fn ensure_keycloak_resources(
        &self,
        instance: &IdentityInstance,
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

        info!(
            name = %name,
            namespace = %namespace,
            provider = "keycloak",
            "Ensuring Keycloak resources"
        );

        let admin_secret_name = keycloak_admin_secret_name(&name);
        self.ensure_keycloak_admin_secret(&namespace, &admin_secret_name)
            .await?;

        let labels = keycloak_labels(instance);
        let deployment =
            build_keycloak_deployment(instance, &name, &namespace, &labels, &admin_secret_name)?;
        let service = build_keycloak_service(instance, &name, &namespace, &labels)?;

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
            "Keycloak resources applied"
        );

        Ok(())
    }

    async fn cleanup_keycloak_resources(
        &self,
        instance: &IdentityInstance,
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
        let delete_params = kube::api::DeleteParams::default();

        info!(
            name = %name,
            namespace = %namespace,
            provider = "keycloak",
            "Cleaning up Keycloak resources"
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
    }
}

impl KubeIdentityInstanceDeployer {
    async fn ensure_keycloak_admin_secret(
        &self,
        namespace: &str,
        secret_name: &str,
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
    let context = Arc::new(OperatorContext {
        service,
        deployer,
        client: client.clone(),
    });

    Controller::new(instances, watcher::Config::default())
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

    match instance.spec.provider {
        aether_crds::v1alpha::identity_instance::IdentityProvider::Keycloak => {
            context
                .deployer
                .cleanup_keycloak_resources(&instance)
                .await?;
        }
        aether_crds::v1alpha::identity_instance::IdentityProvider::Ferriskey => {
            // TODO: nettoyer les ressources Ferriskey.
            unimplemented!("Ferriskey cleanup not implemented yet");
        }
    }

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
) -> Result<Deployment, OperatorError> {
    let image = format!("quay.io/keycloak/keycloak:{}", instance.spec.version);
    let db = &instance.spec.database;
    let credentials_secret = &db.credentials_secret;
    let selector = LabelSelector {
        match_labels: Some(labels.clone()),
        ..Default::default()
    };

    Ok(Deployment {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            labels: Some(labels.clone()),
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
                        args: Some(vec!["start-dev".to_string()]),
                        ports: Some(vec![ContainerPort {
                            container_port: 8080,
                            ..Default::default()
                        }]),
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
) -> Result<Service, OperatorError> {
    Ok(Service {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            labels: Some(labels.clone()),
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
    use aether_crds::v1alpha::identity_instance::{
        DatabaseConfig, IdentityInstance, IdentityInstanceSpec, IdentityProvider,
    };
    use kube::core::ObjectMeta;
    use kube::error::ErrorResponse;
    use kube::{Client, Config};
    use std::sync::Arc;

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
                    host: "postgres.default.svc".to_string(),
                    port: 5432,
                    name: "keycloak_acme".to_string(),
                    credentials_secret: "db-creds".to_string(),
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

    #[tokio::test]
    async fn error_policy_requeues_after_30s() {
        let uri: http::Uri = "http://127.0.0.1:1".parse().unwrap();
        let client = Client::try_from(Config::new(uri)).expect("client");
        let context = Arc::new(OperatorContext {
            service: Arc::new(()),
            deployer: Arc::new(()),
            client,
        });

        let instance = Arc::new(instance());
        let action = error_policy(
            instance,
            &OperatorError::Internal {
                message: "err".to_string(),
            },
            context,
        );

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
        let service = build_keycloak_service(&instance, "instance-1", "default", &labels).unwrap();

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
                .map(|secret| secret.name == "db-creds" && secret.key == "jdbc-uri")
                .unwrap_or(false)
        );
        assert!(
            kc_db_user
                .and_then(|source| source.secret_key_ref.as_ref())
                .map(|secret| secret.name == "db-creds" && secret.key == "user")
                .unwrap_or(false)
        );
        assert!(
            kc_db_pass
                .and_then(|source| source.secret_key_ref.as_ref())
                .map(|secret| secret.name == "db-creds" && secret.key == "password")
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
    }
}
