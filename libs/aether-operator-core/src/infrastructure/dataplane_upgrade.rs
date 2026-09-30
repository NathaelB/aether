use aether_crds::v1alpha::identity_dataplane_upgrade::{
    IdentityDataplaneUpgrade, IdentityDataplaneUpgradeStatus,
};
use k8s_openapi::api::apps::v1::Deployment;
use kube::api::{ListParams, Patch, PatchParams};
use kube::{Api, Client};
use serde_json::{Value, json};

use crate::domain::OperatorError;
use crate::domain::dataplane_upgrade::{ComponentVersions, DataplaneComponentKind};
use crate::domain::ports::{DataplaneUpgradeDeployer, DataplaneUpgradeRepository};

pub const FIELD_MANAGER: &str = "aether-operator-dataplane-upgrade";
const COMPONENT_LABEL: &str = "app.kubernetes.io/component";
const INSTANCE_LABEL: &str = "app.kubernetes.io/instance";

fn kube_error(error: kube::Error) -> OperatorError {
    OperatorError::Kube {
        message: error.to_string(),
    }
}

pub struct KubeDataplaneUpgradeRepository {
    client: Client,
}

impl KubeDataplaneUpgradeRepository {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl DataplaneUpgradeRepository for KubeDataplaneUpgradeRepository {
    async fn get(
        &self,
        name: &str,
        namespace: &str,
    ) -> Result<Option<IdentityDataplaneUpgrade>, OperatorError> {
        let api: Api<IdentityDataplaneUpgrade> = Api::namespaced(self.client.clone(), namespace);
        api.get_opt(name).await.map_err(kube_error)
    }

    async fn patch_status(
        &self,
        name: &str,
        namespace: &str,
        status: IdentityDataplaneUpgradeStatus,
    ) -> Result<(), OperatorError> {
        let api: Api<IdentityDataplaneUpgrade> = Api::namespaced(self.client.clone(), namespace);
        api.patch_status(
            name,
            &PatchParams::default(),
            &Patch::Merge(&status_patch(&status)),
        )
        .await
        .map_err(kube_error)?;
        Ok(())
    }
}

pub struct KubeDataplaneUpgradeDeployer {
    client: Client,
    release: Option<String>,
}

impl KubeDataplaneUpgradeDeployer {
    pub fn new(client: Client, release: Option<String>) -> Self {
        Self { client, release }
    }

    fn selector(&self, component: DataplaneComponentKind) -> String {
        component_selector(component, self.release.as_deref())
    }

    async fn find(
        &self,
        namespace: &str,
        component: DataplaneComponentKind,
    ) -> Result<Option<Deployment>, OperatorError> {
        let api: Api<Deployment> = Api::namespaced(self.client.clone(), namespace);
        let list = api
            .list(&ListParams::default().labels(&self.selector(component)))
            .await
            .map_err(kube_error)?;
        single_deployment(list.items, component)
    }
}

impl DataplaneUpgradeDeployer for KubeDataplaneUpgradeDeployer {
    async fn current_versions(&self, namespace: &str) -> Result<ComponentVersions, OperatorError> {
        let mut versions = ComponentVersions::new();
        for component in DataplaneComponentKind::ALL {
            let Some(deployment) = self.find(namespace, component).await? else {
                continue;
            };
            if let Some(tag) = deployment_version(&deployment, component) {
                versions.insert(component, tag.to_string());
            }
        }
        Ok(versions)
    }

    async fn set_component_version(
        &self,
        namespace: &str,
        component: DataplaneComponentKind,
        version: &str,
    ) -> Result<(), OperatorError> {
        let deployment =
            self.find(namespace, component)
                .await?
                .ok_or_else(|| OperatorError::Internal {
                    message: format!("no deployment found for component `{component}`"),
                })?;
        let name = deployment
            .metadata
            .name
            .as_deref()
            .ok_or(OperatorError::MissingName)?;
        let image =
            component_image(&deployment, component).ok_or_else(|| OperatorError::Internal {
                message: format!("deployment `{name}` has no `{component}` container image"),
            })?;
        let patch = image_patch(name, component, &image_with_tag(image, version));

        let api: Api<Deployment> = Api::namespaced(self.client.clone(), namespace);
        api.patch(
            name,
            &PatchParams::apply(FIELD_MANAGER).force(),
            &Patch::Apply(&patch),
        )
        .await
        .map_err(kube_error)?;
        Ok(())
    }

    async fn component_ready(
        &self,
        namespace: &str,
        component: DataplaneComponentKind,
    ) -> Result<bool, OperatorError> {
        Ok(self
            .find(namespace, component)
            .await?
            .is_some_and(|deployment| rollout_complete(&deployment)))
    }
}

fn component_selector(component: DataplaneComponentKind, release: Option<&str>) -> String {
    match release {
        Some(release) => format!("{COMPONENT_LABEL}={component},{INSTANCE_LABEL}={release}"),
        None => format!("{COMPONENT_LABEL}={component}"),
    }
}

fn single_deployment(
    mut items: Vec<Deployment>,
    component: DataplaneComponentKind,
) -> Result<Option<Deployment>, OperatorError> {
    if items.len() > 1 {
        return Err(OperatorError::Internal {
            message: format!(
                "{} deployments match component `{component}`, expected one",
                items.len()
            ),
        });
    }
    Ok(items.pop())
}

fn component_image(deployment: &Deployment, component: DataplaneComponentKind) -> Option<&str> {
    deployment
        .spec
        .as_ref()?
        .template
        .spec
        .as_ref()?
        .containers
        .iter()
        .find(|container| container.name == component.as_str())?
        .image
        .as_deref()
}

fn deployment_version(deployment: &Deployment, component: DataplaneComponentKind) -> Option<&str> {
    image_tag(component_image(deployment, component)?)
}

fn image_tag(image: &str) -> Option<&str> {
    if image.contains('@') {
        return None;
    }
    let last_segment = image.rsplit('/').next()?;
    let (_, tag) = last_segment.rsplit_once(':')?;
    Some(tag)
}

fn image_with_tag(image: &str, version: &str) -> String {
    let repository = match image_tag(image) {
        Some(tag) => &image[..image.len() - tag.len() - 1],
        None => image.split('@').next().unwrap_or(image),
    };
    format!("{repository}:{version}")
}

fn image_patch(name: &str, component: DataplaneComponentKind, image: &str) -> Value {
    json!({
        "apiVersion": "apps/v1",
        "kind": "Deployment",
        "metadata": { "name": name },
        "spec": {
            "template": {
                "spec": {
                    "containers": [{ "name": component.as_str(), "image": image }]
                }
            }
        }
    })
}

fn status_patch(status: &IdentityDataplaneUpgradeStatus) -> Value {
    json!({ "status": status })
}

fn rollout_complete(deployment: &Deployment) -> bool {
    let desired = deployment
        .spec
        .as_ref()
        .and_then(|spec| spec.replicas)
        .unwrap_or(1);
    let generation = deployment.metadata.generation.unwrap_or_default();
    let Some(status) = deployment.status.as_ref() else {
        return false;
    };
    status.observed_generation.unwrap_or_default() >= generation
        && status.updated_replicas.unwrap_or(0) >= desired
        && status.ready_replicas.unwrap_or(0) >= desired
        && status.available_replicas.unwrap_or(0) >= desired
        && status.replicas.unwrap_or(0) <= status.updated_replicas.unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use aether_crds::v1alpha::identity_dataplane_upgrade::DataplaneUpgradePhase;
    use k8s_openapi::api::apps::v1::{DeploymentSpec, DeploymentStatus};
    use k8s_openapi::api::core::v1::{Container, PodSpec, PodTemplateSpec};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    use super::*;

    fn deployment(containers: &[(&str, &str)]) -> Deployment {
        Deployment {
            metadata: ObjectMeta {
                name: Some("aether-herald".to_string()),
                generation: Some(2),
                ..Default::default()
            },
            spec: Some(DeploymentSpec {
                replicas: Some(2),
                template: PodTemplateSpec {
                    spec: Some(PodSpec {
                        containers: containers
                            .iter()
                            .map(|(name, image)| Container {
                                name: name.to_string(),
                                image: Some(image.to_string()),
                                ..Default::default()
                            })
                            .collect(),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn with_status(mut deployment: Deployment, status: DeploymentStatus) -> Deployment {
        deployment.status = Some(status);
        deployment
    }

    fn settled() -> DeploymentStatus {
        DeploymentStatus {
            observed_generation: Some(2),
            replicas: Some(2),
            updated_replicas: Some(2),
            ready_replicas: Some(2),
            available_replicas: Some(2),
            ..Default::default()
        }
    }

    #[test]
    fn tag_is_read_from_the_last_path_segment() {
        assert_eq!(image_tag("ghcr.io/org/aether-herald:0.4.0"), Some("0.4.0"));
        assert_eq!(
            image_tag("localhost:5000/aether-herald:0.4.0"),
            Some("0.4.0")
        );
    }

    #[test]
    fn registry_port_is_not_a_tag() {
        assert_eq!(image_tag("localhost:5000/aether-herald"), None);
    }

    #[test]
    fn digest_pinned_image_has_no_tag() {
        assert_eq!(image_tag("ghcr.io/org/aether-herald@sha256:abc"), None);
    }

    #[test]
    fn version_replaces_the_tag_and_keeps_the_repository() {
        assert_eq!(
            image_with_tag("localhost:5000/org/aether-herald:0.4.0", "0.5.0"),
            "localhost:5000/org/aether-herald:0.5.0"
        );
    }

    #[test]
    fn version_is_appended_to_an_untagged_image() {
        assert_eq!(
            image_with_tag("localhost:5000/aether-herald", "0.5.0"),
            "localhost:5000/aether-herald:0.5.0"
        );
    }

    #[test]
    fn version_replaces_a_digest() {
        assert_eq!(
            image_with_tag("ghcr.io/org/aether-herald@sha256:abc", "0.5.0"),
            "ghcr.io/org/aether-herald:0.5.0"
        );
    }

    #[test]
    fn container_is_selected_by_component_name() {
        let deployment = deployment(&[
            ("sidecar", "proxy:1.0"),
            ("herald", "ghcr.io/org/aether-herald:0.4.0"),
        ]);

        assert_eq!(
            component_image(&deployment, DataplaneComponentKind::Herald),
            Some("ghcr.io/org/aether-herald:0.4.0")
        );
        assert_eq!(
            deployment_version(&deployment, DataplaneComponentKind::Herald),
            Some("0.4.0")
        );
        assert_eq!(
            component_image(&deployment, DataplaneComponentKind::Genesis),
            None
        );
    }

    #[test]
    fn selector_uses_component_and_optional_release() {
        assert_eq!(
            component_selector(DataplaneComponentKind::Genesis, None),
            "app.kubernetes.io/component=genesis"
        );
        assert_eq!(
            component_selector(DataplaneComponentKind::Genesis, Some("aether")),
            "app.kubernetes.io/component=genesis,app.kubernetes.io/instance=aether"
        );
    }

    #[test]
    fn more_than_one_match_is_an_error() {
        let items = vec![deployment(&[]), deployment(&[])];

        assert!(single_deployment(items, DataplaneComponentKind::Herald).is_err());
        assert!(
            single_deployment(Vec::new(), DataplaneComponentKind::Herald)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn image_patch_touches_only_the_component_container_image() {
        let patch = image_patch(
            "aether-herald",
            DataplaneComponentKind::Herald,
            "ghcr.io/org/aether-herald:0.5.0",
        );

        assert_eq!(
            patch,
            json!({
                "apiVersion": "apps/v1",
                "kind": "Deployment",
                "metadata": { "name": "aether-herald" },
                "spec": { "template": { "spec": { "containers": [
                    { "name": "herald", "image": "ghcr.io/org/aether-herald:0.5.0" }
                ] } } }
            })
        );
    }

    #[test]
    fn status_patch_wraps_the_status() {
        let status = IdentityDataplaneUpgradeStatus {
            phase: DataplaneUpgradePhase::Upgrading,
            current_version: Some("0.4.0".to_string()),
            progress: Some("1/3".to_string()),
            conditions: Vec::new(),
            components: Vec::new(),
        };

        assert_eq!(
            status_patch(&status),
            json!({ "status": {
                "phase": "Upgrading",
                "currentVersion": "0.4.0",
                "progress": "1/3"
            } })
        );
    }

    #[test]
    fn settled_rollout_is_ready() {
        assert!(rollout_complete(&with_status(deployment(&[]), settled())));
    }

    #[test]
    fn deployment_without_status_is_not_ready() {
        assert!(!rollout_complete(&deployment(&[])));
    }

    #[test]
    fn stale_generation_is_not_ready() {
        let status = DeploymentStatus {
            observed_generation: Some(1),
            ..settled()
        };

        assert!(!rollout_complete(&with_status(deployment(&[]), status)));
    }

    #[test]
    fn pods_still_on_the_old_template_are_not_ready() {
        let status = DeploymentStatus {
            replicas: Some(3),
            updated_replicas: Some(1),
            ..settled()
        };

        assert!(!rollout_complete(&with_status(deployment(&[]), status)));
    }

    #[test]
    fn unready_pods_are_not_ready() {
        let status = DeploymentStatus {
            ready_replicas: Some(1),
            ..settled()
        };

        assert!(!rollout_complete(&with_status(deployment(&[]), status)));
    }
}
