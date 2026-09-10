use std::sync::Arc;
use std::time::Duration;

use aether_crds::common::types::Phase;
use aether_crds::v1alpha::identity_instance::{IdentityInstance, IdentityInstanceStatus};
use aether_crds::v1alpha::identity_instance_upgrade::{
    IdentityInstanceUpgrade, IdentityInstanceUpgradeStatus,
};
use futures::StreamExt;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use k8s_openapi::chrono::Utc;
use kube::runtime::controller::{Action, Controller};
use kube::runtime::events::{Event as KubeEvent, EventType, Recorder, Reporter};
use kube::runtime::watcher;
use kube::{Api, Client, Resource};
use serde_json::json;
use tracing::{error, info, warn};

use crate::domain::OperatorError;

/// How long an upgrade may sit unhealthy before the controller stops retrying and marks it
/// failed. Sized to cover a Keycloak/FerrisKey rolling restart plus a slow database migration
/// on modest hardware, while still surfacing a stuck upgrade within a working day rather than
/// spinning silently forever. A per-upgrade override (spec field or CLI flag) was considered
/// and rejected: the operator has no configuration plumbing today, and adding one for a single
/// knob is out of scope for this fix.
pub const UPGRADE_DEADLINE: Duration = Duration::from_secs(30 * 60);

#[derive(Clone)]
struct UpgradeContext {
    client: Client,
}

pub async fn run() -> Result<(), OperatorError> {
    info!("Starting IdentityInstanceUpgrade controller");
    let client = Client::try_default()
        .await
        .map_err(|error| OperatorError::Kube {
            message: error.to_string(),
        })?;

    let upgrades = Api::<IdentityInstanceUpgrade>::all(client.clone());
    let context = Arc::new(UpgradeContext { client });

    Controller::new(upgrades, watcher::Config::default())
        .run(reconcile, error_policy, context)
        .for_each(|_| async {})
        .await;

    Ok(())
}

async fn reconcile(
    upgrade: Arc<IdentityInstanceUpgrade>,
    context: Arc<UpgradeContext>,
) -> Result<Action, OperatorError> {
    let name = upgrade.metadata.name.clone().unwrap_or_default();
    let namespace = upgrade.metadata.namespace.clone().unwrap_or_default();
    info!(
        name = %name,
        namespace = %namespace,
        "Reconciling IdentityInstanceUpgrade"
    );

    let namespace = upgrade
        .metadata
        .namespace
        .clone()
        .ok_or_else(|| OperatorError::MissingNamespace { name: name.clone() })?;
    let upgrades: Api<IdentityInstanceUpgrade> =
        Api::namespaced(context.client.clone(), &namespace);
    let instances: Api<IdentityInstance> = Api::namespaced(context.client.clone(), &namespace);
    let current_status = upgrade.status.clone().unwrap_or_default();
    let upgrade_name = name.clone();

    if current_status.completed && current_status.pending_cleanup {
        upgrades
            .delete(&upgrade_name, &kube::api::DeleteParams::default())
            .await
            .map_err(|error| OperatorError::Kube {
                message: error.to_string(),
            })?;
        info!(
            name = %upgrade_name,
            namespace = %namespace,
            "IdentityInstanceUpgrade completed and deleted"
        );
        return Ok(Action::await_change());
    }

    if !upgrade.spec.approved {
        let desired = IdentityInstanceUpgradeStatus {
            phase: Some(Phase::Pending),
            completed: false,
            current_version: current_status.current_version.clone(),
            target_version: Some(upgrade.spec.target_version.clone()),
            started_at: current_status.started_at.clone(),
            completed_at: None,
            pending_cleanup: false,
            conditions: current_status.conditions.clone(),
            message: Some("Waiting for approval before starting upgrade.".to_string()),
            error: None,
        };
        patch_upgrade_status_if_changed(&context.client, &upgrades, &upgrade, desired).await?;
        return Ok(Action::await_change());
    }

    let instance_name = upgrade.spec.identity_instance_ref.name.clone();
    let instance = instances
        .get(&instance_name)
        .await
        .map_err(|error| OperatorError::Kube {
            message: error.to_string(),
        })?;

    patch_identity_instance_status_if_changed(&instances, &instance, Phase::Upgrading, false)
        .await?;

    let mut spec_was_patched = false;
    let updated_instance = if instance.spec.version != upgrade.spec.target_version {
        spec_was_patched = true;
        info!(
            upgrade = %name,
            instance = %instance_name,
            from = %instance.spec.version,
            to = %upgrade.spec.target_version,
            "Applying target version to IdentityInstance"
        );

        let patch = json!({
            "spec": {
                "version": upgrade.spec.target_version
            }
        });

        instances
            .patch(
                &instance_name,
                &kube::api::PatchParams::default(),
                &kube::api::Patch::Merge(&patch),
            )
            .await
            .map_err(|error| OperatorError::Kube {
                message: error.to_string(),
            })?
    } else {
        instance
    };

    if spec_was_patched {
        patch_identity_instance_status_if_changed(
            &instances,
            &updated_instance,
            Phase::Upgrading,
            false,
        )
        .await?;

        let desired = IdentityInstanceUpgradeStatus {
            phase: Some(Phase::Updating),
            completed: false,
            current_version: Some(updated_instance.spec.version.clone()),
            target_version: Some(upgrade.spec.target_version.clone()),
            started_at: current_status
                .started_at
                .clone()
                .or_else(|| Some(Time(Utc::now()))),
            completed_at: None,
            pending_cleanup: false,
            conditions: current_status.conditions.clone(),
            message: Some(format!(
                "Upgrade started: target version {}.",
                upgrade.spec.target_version
            )),
            error: None,
        };
        patch_upgrade_status_if_changed(&context.client, &upgrades, &upgrade, desired).await?;
        return Ok(Action::requeue(Duration::from_secs(15)));
    }

    let upgrade_completed = updated_instance.spec.version == upgrade.spec.target_version
        && identity_instance_runtime_ready(
            &context.client,
            &updated_instance,
            &upgrade.spec.target_version,
        )
        .await?;

    if upgrade_completed {
        patch_identity_instance_status_if_changed(
            &instances,
            &updated_instance,
            Phase::Running,
            true,
        )
        .await?;

        let desired = IdentityInstanceUpgradeStatus {
            phase: Some(Phase::Running),
            completed: true,
            current_version: Some(updated_instance.spec.version.clone()),
            target_version: Some(upgrade.spec.target_version.clone()),
            started_at: current_status.started_at.clone(),
            completed_at: current_status
                .completed_at
                .clone()
                .or_else(|| Some(Time(Utc::now()))),
            pending_cleanup: true,
            conditions: current_status.conditions.clone(),
            message: Some(format!(
                "Upgrade completed successfully to version {}.",
                upgrade.spec.target_version
            )),
            error: None,
        };
        patch_upgrade_status_if_changed(&context.client, &upgrades, &upgrade, desired).await?;

        return Ok(Action::requeue(Duration::from_secs(30)));
    }

    if upgrade_deadline_exceeded(&current_status, UPGRADE_DEADLINE) {
        patch_identity_instance_status_if_changed(
            &instances,
            &updated_instance,
            Phase::Failed,
            false,
        )
        .await?;

        let desired = IdentityInstanceUpgradeStatus {
            phase: Some(Phase::Failed),
            completed: false,
            current_version: Some(updated_instance.spec.version.clone()),
            target_version: Some(upgrade.spec.target_version.clone()),
            started_at: current_status.started_at.clone(),
            completed_at: None,
            pending_cleanup: false,
            conditions: current_status.conditions.clone(),
            message: Some(format!(
                "Upgrade deadline exceeded before the instance became healthy on target version {}.",
                upgrade.spec.target_version
            )),
            error: Some(format!(
                "no healthy deployment for target version {} within {} seconds of upgrade start",
                upgrade.spec.target_version,
                UPGRADE_DEADLINE.as_secs()
            )),
        };
        patch_upgrade_status_if_changed(&context.client, &upgrades, &upgrade, desired).await?;

        // No further periodic retry: the instance is stuck on a version that will not become
        // ready on its own, and rolling it back automatically is a separate concern (#105).
        // A new reconcile only happens once the spec or status changes again.
        return Ok(Action::await_change());
    }

    patch_identity_instance_status_if_changed(
        &instances,
        &updated_instance,
        Phase::Upgrading,
        false,
    )
    .await?;

    let desired = IdentityInstanceUpgradeStatus {
        phase: Some(Phase::Updating),
        completed: false,
        current_version: Some(updated_instance.spec.version.clone()),
        target_version: Some(upgrade.spec.target_version.clone()),
        started_at: current_status
            .started_at
            .clone()
            .or_else(|| Some(Time(Utc::now()))),
        completed_at: None,
        pending_cleanup: false,
        conditions: current_status.conditions.clone(),
        message: Some(format!(
            "Upgrade in progress: target version {}.",
            upgrade.spec.target_version
        )),
        error: None,
    };
    patch_upgrade_status_if_changed(&context.client, &upgrades, &upgrade, desired).await?;

    Ok(Action::requeue(Duration::from_secs(15)))
}

/// True once more than `deadline` has elapsed since the upgrade started without it having
/// become healthy. An upgrade with no recorded start (should not happen once approved, but
/// defensive against a status wiped out of band) is never considered overdue.
fn upgrade_deadline_exceeded(status: &IdentityInstanceUpgradeStatus, deadline: Duration) -> bool {
    let Some(started_at) = status.started_at.as_ref() else {
        return false;
    };

    Utc::now()
        .signed_duration_since(started_at.0)
        .to_std()
        .map(|elapsed| elapsed > deadline)
        .unwrap_or(false)
}

fn error_policy(
    _upgrade: Arc<IdentityInstanceUpgrade>,
    error: &OperatorError,
    _context: Arc<UpgradeContext>,
) -> Action {
    error!(error = %error, "IdentityInstanceUpgrade reconcile error");
    Action::requeue(Duration::from_secs(30))
}

async fn patch_upgrade_status_if_changed(
    client: &Client,
    api: &Api<IdentityInstanceUpgrade>,
    upgrade: &IdentityInstanceUpgrade,
    desired_status: IdentityInstanceUpgradeStatus,
) -> Result<(), OperatorError> {
    let current_status = upgrade.status.clone().unwrap_or_default();
    if current_status == desired_status {
        return Ok(());
    }

    let name = upgrade
        .metadata
        .name
        .clone()
        .ok_or(OperatorError::MissingName)?;
    let patch = json!({ "status": desired_status.clone() });

    api.patch_status(
        &name,
        &kube::api::PatchParams::default(),
        &kube::api::Patch::Merge(&patch),
    )
    .await
    .map_err(|error| OperatorError::Kube {
        message: error.to_string(),
    })?;

    if let Err(error) =
        publish_upgrade_event(client, upgrade, &current_status, &desired_status).await
    {
        warn!(
            upgrade = %name,
            error = %error,
            "Failed to publish IdentityInstanceUpgrade event"
        );
    }

    Ok(())
}

async fn publish_upgrade_event(
    client: &Client,
    upgrade: &IdentityInstanceUpgrade,
    previous: &IdentityInstanceUpgradeStatus,
    current: &IdentityInstanceUpgradeStatus,
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

    let phase_reason = match current.phase.clone() {
        Some(Phase::Pending) => "UpgradePendingApproval",
        Some(Phase::Updating) => "UpgradeInProgress",
        Some(Phase::Running) => "UpgradeCompleted",
        _ => "UpgradeStatusUpdated",
    };

    let reporter = Reporter {
        controller: "aether-operator".to_string(),
        instance: Some("identityinstance-upgrade-controller".to_string()),
    };
    let recorder = Recorder::new(client.clone(), reporter);
    let reference = upgrade.object_ref(&());
    let note = current.message.clone().unwrap_or_else(|| {
        format!(
            "Upgrade transition: phase {previous_phase} -> {current_phase}, ready {} -> {}",
            previous.completed, current.completed
        )
    });

    let event = KubeEvent {
        type_: EventType::Normal,
        reason: phase_reason.to_string(),
        note: Some(note),
        action: "UpgradeReconcile".to_string(),
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

async fn patch_identity_instance_status_if_changed(
    api: &Api<IdentityInstance>,
    instance: &IdentityInstance,
    phase: Phase,
    ready: bool,
) -> Result<(), OperatorError> {
    let name = instance
        .metadata
        .name
        .clone()
        .ok_or(OperatorError::MissingName)?;
    let current_status = instance.status.clone().unwrap_or_default();
    let mut desired_status: IdentityInstanceStatus = current_status.clone();
    desired_status.phase = Some(phase);
    desired_status.ready = ready;

    if desired_status == current_status {
        return Ok(());
    }

    let patch = json!({ "status": desired_status });
    api.patch_status(
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

async fn identity_instance_runtime_ready(
    client: &Client,
    instance: &IdentityInstance,
    target_version: &str,
) -> Result<bool, OperatorError> {
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
    let deployments: Api<Deployment> = Api::namespaced(client.clone(), &namespace);

    let deployment = deployments
        .get_opt(&name)
        .await
        .map_err(|error| OperatorError::Kube {
            message: error.to_string(),
        })?;
    let Some(deployment) = deployment else {
        return Ok(false);
    };

    let deployment_has_target_version = deployment
        .spec
        .as_ref()
        .and_then(|spec| spec.template.spec.as_ref())
        .map(|pod_spec| {
            pod_spec.containers.iter().any(|container| {
                container.name == "keycloak"
                    && container
                        .image
                        .as_deref()
                        .map(|image| image.ends_with(&format!(":{target_version}")))
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false);

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

    Ok(deployment_has_target_version
        && observed_generation >= generation
        && ready_replicas >= desired_replicas
        && available_replicas >= desired_replicas)
}

#[cfg(test)]
mod tests {
    use k8s_openapi::chrono::Duration as ChronoDuration;

    use super::*;

    fn status_started(elapsed_ago: ChronoDuration) -> IdentityInstanceUpgradeStatus {
        IdentityInstanceUpgradeStatus {
            started_at: Some(Time(Utc::now() - elapsed_ago)),
            ..Default::default()
        }
    }

    #[test]
    fn an_upgrade_started_within_the_deadline_is_not_considered_overdue() {
        let status = status_started(ChronoDuration::minutes(5));

        assert!(!upgrade_deadline_exceeded(
            &status,
            Duration::from_secs(30 * 60)
        ));
    }

    #[test]
    fn an_upgrade_that_never_becomes_healthy_is_failed_rather_than_retried_for_ever() {
        let status = status_started(ChronoDuration::minutes(45));

        assert!(upgrade_deadline_exceeded(
            &status,
            Duration::from_secs(30 * 60)
        ));
    }

    #[test]
    fn an_upgrade_with_no_recorded_start_is_never_considered_overdue() {
        let status = IdentityInstanceUpgradeStatus::default();

        assert!(!upgrade_deadline_exceeded(&status, Duration::from_secs(1)));
    }
}
