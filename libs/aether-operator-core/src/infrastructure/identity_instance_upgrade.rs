use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use aether_crds::common::types::Phase;
use aether_crds::v1alpha::identity_instance::{
    IdentityInstance, IdentityInstanceStatus, IdentityProvider,
};
use aether_crds::v1alpha::identity_instance_upgrade::{
    IdentityInstanceUpgrade, IdentityInstanceUpgradeStatus, UpgradeOutcome,
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
use crate::infrastructure::identity_instance;

/// How long an upgrade may sit unhealthy before the controller stops retrying and marks it
/// failed. Sized to cover a Keycloak/FerrisKey rolling restart plus a slow database migration
/// on modest hardware, while still surfacing a stuck upgrade within a working day rather than
/// spinning silently forever. A per-upgrade override (spec field or CLI flag) was considered
/// and rejected: the operator has no configuration plumbing today, and adding one for a single
/// knob is out of scope for this fix.
pub const UPGRADE_DEADLINE: Duration = Duration::from_secs(30 * 60);

/// How long a structurally ready instance (right image, right replica counts) may fail to
/// answer a health check before the controller concludes the product itself, not just the
/// pod, is broken. Kept far shorter than `UPGRADE_DEADLINE`: a process that has just restarted
/// is briefly unreachable while it warms up, but one that answers nothing for several minutes
/// after Kubernetes already calls it ready is not warming up.
pub const HEALTH_CHECK_GRACE_WINDOW: Duration = Duration::from_secs(3 * 60);

/// How long a single health check attempt is given to answer before it counts as a miss.
const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(5);

type HealthCheckFuture<'a> = Pin<Box<dyn Future<Output = bool> + Send + 'a>>;

/// Crosses the network to ask whether the product behind an instance is answering, as
/// opposed to merely running. Kept behind a trait so the decision in `evaluate_runtime_health`
/// can be exercised without a socket.
trait HealthCheck: Send + Sync {
    fn probe<'a>(&'a self, url: &'a str) -> HealthCheckFuture<'a>;
}

struct HttpHealthCheck {
    client: reqwest::Client,
}

impl HttpHealthCheck {
    fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(HEALTH_CHECK_TIMEOUT)
            .build()
            .unwrap_or_default();
        Self { client }
    }
}

impl HealthCheck for HttpHealthCheck {
    fn probe<'a>(&'a self, url: &'a str) -> HealthCheckFuture<'a> {
        let client = self.client.clone();
        let url = url.to_string();
        Box::pin(async move {
            // A server error means the process is up and refusing the request, which is
            // exactly the "ready but broken" case this check exists to catch. Anything short
            // of that (2xx, a redirect, even a 4xx) proves the product is answering HTTP.
            match client.get(&url).send().await {
                Ok(response) => !response.status().is_server_error(),
                Err(_) => false,
            }
        })
    }
}

#[derive(Clone)]
struct UpgradeContext {
    client: Client,
    health_check: Arc<dyn HealthCheck>,
}

pub async fn run() -> Result<(), OperatorError> {
    info!("Starting IdentityInstanceUpgrade controller");
    let client = Client::try_default()
        .await
        .map_err(|error| OperatorError::Kube {
            message: error.to_string(),
        })?;

    let upgrades = Api::<IdentityInstanceUpgrade>::all(client.clone());
    let context = Arc::new(UpgradeContext {
        client,
        health_check: Arc::new(HttpHealthCheck::new()),
    });

    Controller::new(upgrades, watcher::Config::default())
        .run(reconcile, error_policy, context)
        .for_each(|_| async {})
        .await;

    Ok(())
}

/// The one place the controller decides whether the instance it just checked, on whichever
/// version it is currently pursuing, is done, still settling in, or broken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeHealth {
    NotReady,
    StillWarmingUp,
    Unresponsive,
    Healthy,
}

/// Pure by design: everything it needs is already known, so it can be exercised without a
/// cluster or a socket. `ready_since` absent (the deployment just became ready this reconcile)
/// is treated the same as "inside the grace window", since there has been no time to measure
/// against yet.
///
/// `probe` is `None` when no in-cluster target could even be built (missing name or
/// namespace on the instance). That is not evidence of anything: it must not, by itself, ever
/// turn into a rollback of an instance that Kubernetes already calls ready. Readiness alone,
/// the check this file used before, is the honest fallback there.
fn evaluate_runtime_health(
    deployment_ready: bool,
    probe: Option<bool>,
    ready_since: Option<&Time>,
    grace_window: Duration,
) -> RuntimeHealth {
    if !deployment_ready {
        return RuntimeHealth::NotReady;
    }
    match probe {
        None | Some(true) => RuntimeHealth::Healthy,
        Some(false) => match ready_since {
            Some(since) if elapsed_exceeds(since, grace_window) => RuntimeHealth::Unresponsive,
            _ => RuntimeHealth::StillWarmingUp,
        },
    }
}

/// Why the attempt currently being pursued (the upgrade itself, or a rollback of it) was
/// declared failed. Kept distinct because "not ready" and "ready but not answering" point at
/// different problems: one is Kubernetes never scheduling healthy pods, the other is the
/// product rejecting traffic once it is up.
enum FailureReason {
    NotReady,
    Unresponsive,
}

impl FailureReason {
    fn describe(&self, version: &str) -> String {
        match self {
            Self::NotReady => format!(
                "instance did not become ready on version {version} within {} seconds",
                UPGRADE_DEADLINE.as_secs()
            ),
            Self::Unresponsive => format!(
                "instance reported ready on version {version} but did not answer a health check within {} seconds of becoming ready",
                HEALTH_CHECK_GRACE_WINDOW.as_secs()
            ),
        }
    }
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

    // A terminal outcome is final. Without this guard, a watch event fired by our own status
    // patch (its resourceVersion changes too) would re-enter the logic below, see the spec
    // still pointing away from the target version, and start the whole dance over.
    if current_status.outcome.is_some() {
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
            ..Default::default()
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

    let rolling_back = current_status.rollback_started_at.is_some();
    let desired_version = if rolling_back {
        current_status
            .previous_version
            .clone()
            .unwrap_or_else(|| upgrade.spec.target_version.clone())
    } else {
        upgrade.spec.target_version.clone()
    };

    let mut spec_was_patched = false;
    let mut starting_version = None;
    let updated_instance = if instance.spec.version != desired_version {
        spec_was_patched = true;
        starting_version = Some(instance.spec.version.clone());
        info!(
            upgrade = %name,
            instance = %instance_name,
            from = %instance.spec.version,
            to = %desired_version,
            rolling_back,
            "Applying version to IdentityInstance"
        );

        let patch = json!({
            "spec": {
                "version": desired_version
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

        // The starting version is recorded here, once, from the live resource captured just
        // before this same patch overwrote it: it is the only moment that value is still
        // visible anywhere. A rollback reaching this block moves the spec a second time, but
        // must not overwrite the value recorded on the first, forward pass.
        let previous_version = if rolling_back {
            current_status.previous_version.clone()
        } else {
            current_status.previous_version.clone().or(starting_version)
        };

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
                desired_version
            )),
            error: current_status.error.clone(),
            previous_version,
            runtime_ready_since: None,
            rollback_started_at: current_status.rollback_started_at.clone(),
            outcome: None,
        };
        patch_upgrade_status_if_changed(&context.client, &upgrades, &upgrade, desired).await?;
        return Ok(Action::requeue(Duration::from_secs(15)));
    }

    let deployment_ready =
        deployment_ready_for_version(&context.client, &updated_instance, &desired_version).await?;
    let probe = if deployment_ready {
        match instance_health_probe_url(&updated_instance) {
            Some(url) => Some(context.health_check.probe(&url).await),
            // No Service target could be built (missing name or namespace, which should not
            // happen for a resource we just fetched). Nothing to base a rollback on: fall
            // back to readiness alone, same as before this check existed.
            None => None,
        }
    } else {
        None
    };

    let health = evaluate_runtime_health(
        deployment_ready,
        probe,
        current_status.runtime_ready_since.as_ref(),
        HEALTH_CHECK_GRACE_WINDOW,
    );

    match health {
        RuntimeHealth::Healthy => {
            return finalize_success(
                &context,
                &upgrades,
                &upgrade,
                &instances,
                &updated_instance,
                &current_status,
                rolling_back,
            )
            .await;
        }
        RuntimeHealth::Unresponsive => {
            return handle_failed_attempt(
                &context,
                &upgrades,
                &upgrade,
                &instances,
                &updated_instance,
                &current_status,
                rolling_back,
                FailureReason::Unresponsive,
                &desired_version,
            )
            .await;
        }
        RuntimeHealth::NotReady => {
            let deadline_exceeded = if rolling_back {
                deadline_exceeded_since(
                    current_status.rollback_started_at.as_ref(),
                    UPGRADE_DEADLINE,
                )
            } else {
                upgrade_deadline_exceeded(&current_status, UPGRADE_DEADLINE)
            };
            if deadline_exceeded {
                return handle_failed_attempt(
                    &context,
                    &upgrades,
                    &upgrade,
                    &instances,
                    &updated_instance,
                    &current_status,
                    rolling_back,
                    FailureReason::NotReady,
                    &desired_version,
                )
                .await;
            }
        }
        RuntimeHealth::StillWarmingUp => {}
    }

    let runtime_ready_since = match health {
        RuntimeHealth::NotReady => None,
        _ => Some(
            current_status
                .runtime_ready_since
                .clone()
                .unwrap_or_else(|| Time(Utc::now())),
        ),
    };

    let message = match health {
        RuntimeHealth::StillWarmingUp => format!(
            "Instance is ready on version {desired_version}; waiting for it to answer a health check."
        ),
        _ => format!("Upgrade in progress: target version {}.", desired_version),
    };

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
        message: Some(message),
        error: current_status.error.clone(),
        previous_version: current_status.previous_version.clone(),
        runtime_ready_since,
        rollback_started_at: current_status.rollback_started_at.clone(),
        outcome: None,
    };
    patch_upgrade_status_if_changed(&context.client, &upgrades, &upgrade, desired).await?;

    Ok(Action::requeue(Duration::from_secs(15)))
}

#[allow(clippy::too_many_arguments)]
async fn finalize_success(
    context: &UpgradeContext,
    upgrades: &Api<IdentityInstanceUpgrade>,
    upgrade: &IdentityInstanceUpgrade,
    instances: &Api<IdentityInstance>,
    updated_instance: &IdentityInstance,
    current_status: &IdentityInstanceUpgradeStatus,
    rolling_back: bool,
) -> Result<Action, OperatorError> {
    if rolling_back {
        patch_identity_instance_status_if_changed(
            instances,
            updated_instance,
            Phase::Running,
            true,
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
                "Upgrade to {} failed; rolled back to {} and confirmed healthy.",
                upgrade.spec.target_version, updated_instance.spec.version
            )),
            error: current_status.error.clone(),
            previous_version: current_status.previous_version.clone(),
            runtime_ready_since: None,
            rollback_started_at: current_status.rollback_started_at.clone(),
            outcome: Some(UpgradeOutcome::RolledBack),
        };
        patch_upgrade_status_if_changed(&context.client, upgrades, upgrade, desired).await?;
        return Ok(Action::await_change());
    }

    patch_identity_instance_status_if_changed(instances, updated_instance, Phase::Running, true)
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
        previous_version: current_status.previous_version.clone(),
        runtime_ready_since: current_status.runtime_ready_since.clone(),
        rollback_started_at: None,
        outcome: None,
    };
    patch_upgrade_status_if_changed(&context.client, upgrades, upgrade, desired).await?;

    Ok(Action::requeue(Duration::from_secs(30)))
}

/// Reached when the attempt currently being pursued (the upgrade, or a rollback of it) has
/// been declared failed. Starts a rollback if one is not already underway and a distinct
/// earlier version is on record; otherwise this is as far as automation can take it, and the
/// upgrade ends in an explicit failed state naming every version involved.
#[allow(clippy::too_many_arguments)]
async fn handle_failed_attempt(
    context: &UpgradeContext,
    upgrades: &Api<IdentityInstanceUpgrade>,
    upgrade: &IdentityInstanceUpgrade,
    instances: &Api<IdentityInstance>,
    updated_instance: &IdentityInstance,
    current_status: &IdentityInstanceUpgradeStatus,
    rolling_back: bool,
    reason: FailureReason,
    attempted_version: &str,
) -> Result<Action, OperatorError> {
    if rolling_back {
        let previous = current_status
            .previous_version
            .clone()
            .unwrap_or_else(|| attempted_version.to_string());
        let target = upgrade.spec.target_version.clone();

        patch_identity_instance_status_if_changed(
            instances,
            updated_instance,
            Phase::Failed,
            false,
        )
        .await?;

        let desired = IdentityInstanceUpgradeStatus {
            phase: Some(Phase::Failed),
            completed: false,
            current_version: Some(updated_instance.spec.version.clone()),
            target_version: Some(target.clone()),
            started_at: current_status.started_at.clone(),
            completed_at: None,
            pending_cleanup: false,
            conditions: current_status.conditions.clone(),
            message: Some(format!(
                "Upgrade to {target} failed and the rollback to {previous} also failed: {}.",
                reason.describe(&previous)
            )),
            error: Some(format!(
                "upgrade to {target} failed; rollback to {previous} failed too ({})",
                reason.describe(&previous)
            )),
            previous_version: current_status.previous_version.clone(),
            runtime_ready_since: None,
            rollback_started_at: current_status.rollback_started_at.clone(),
            outcome: Some(UpgradeOutcome::Failed),
        };
        patch_upgrade_status_if_changed(&context.client, upgrades, upgrade, desired).await?;
        return Ok(Action::await_change());
    }

    let target = upgrade.spec.target_version.clone();
    let restorable_previous = current_status
        .previous_version
        .clone()
        .filter(|version| !version.is_empty() && version != &target);

    let Some(previous) = restorable_previous else {
        patch_identity_instance_status_if_changed(
            instances,
            updated_instance,
            Phase::Failed,
            false,
        )
        .await?;

        let desired = IdentityInstanceUpgradeStatus {
            phase: Some(Phase::Failed),
            completed: false,
            current_version: Some(updated_instance.spec.version.clone()),
            target_version: Some(target.clone()),
            started_at: current_status.started_at.clone(),
            completed_at: None,
            pending_cleanup: false,
            conditions: current_status.conditions.clone(),
            message: Some(format!(
                "Upgrade to {target} failed and no earlier version was recorded to restore."
            )),
            error: Some(reason.describe(&target)),
            previous_version: current_status.previous_version.clone(),
            runtime_ready_since: None,
            rollback_started_at: None,
            outcome: Some(UpgradeOutcome::Failed),
        };
        patch_upgrade_status_if_changed(&context.client, upgrades, upgrade, desired).await?;
        return Ok(Action::await_change());
    };

    let desired = IdentityInstanceUpgradeStatus {
        phase: Some(Phase::Upgrading),
        completed: false,
        current_version: Some(updated_instance.spec.version.clone()),
        target_version: Some(target.clone()),
        started_at: current_status.started_at.clone(),
        completed_at: None,
        pending_cleanup: false,
        conditions: current_status.conditions.clone(),
        message: Some(format!(
            "Upgrade to {target} failed ({}); rolling back to {previous}.",
            reason.describe(&target)
        )),
        error: Some(reason.describe(&target)),
        previous_version: current_status.previous_version.clone(),
        runtime_ready_since: None,
        rollback_started_at: Some(Time(Utc::now())),
        outcome: None,
    };
    patch_upgrade_status_if_changed(&context.client, upgrades, upgrade, desired).await?;

    Ok(Action::requeue(Duration::from_secs(15)))
}

/// True once more than `deadline` has elapsed since the upgrade started without it having
/// become healthy. An upgrade with no recorded start (should not happen once approved, but
/// defensive against a status wiped out of band) is never considered overdue.
fn upgrade_deadline_exceeded(status: &IdentityInstanceUpgradeStatus, deadline: Duration) -> bool {
    deadline_exceeded_since(status.started_at.as_ref(), deadline)
}

fn deadline_exceeded_since(since: Option<&Time>, deadline: Duration) -> bool {
    let Some(since) = since else {
        return false;
    };
    elapsed_exceeds(since, deadline)
}

fn elapsed_exceeds(since: &Time, window: Duration) -> bool {
    Utc::now()
        .signed_duration_since(since.0)
        .to_std()
        .map(|elapsed| elapsed > window)
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
        Some(Phase::Failed) => "UpgradeFailed",
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

/// The in-cluster Service URL to probe. Deliberately not `status.endpoint`: that is the
/// public hostname, reached only through DNS, TLS and an ingress the operator does not
/// control, so a failure there proves nothing about the product and would turn unrelated
/// infrastructure hiccups (DNS not yet propagated, a certificate still issuing, an ingress
/// not yet programmed) into automatic rollbacks of perfectly healthy upgrades.
///
/// One Service per provider, addressed by its cluster-local `.svc` name rather than a
/// namespace-qualified one, so the result resolves the same regardless of which namespace the
/// operator itself runs in:
/// - Keycloak: the instance's own Service, port 80 (see `build_keycloak_service`).
/// - FerrisKey: the `{instance}-api` Service, port 3333 (see `build_ferriskey_service`); the
///   webapp Service is a front end and answering does not say anything about the API.
///
/// The name and the port both come from `identity_instance.rs`, which is what creates the
/// Service. A probe pointed at a name or a port nothing answers on reads as a product that is
/// not serving, so a second copy of either would turn a rename into a rollback.
fn instance_health_probe_url(instance: &IdentityInstance) -> Option<String> {
    let name = instance.metadata.name.as_deref()?;
    let namespace = instance.metadata.namespace.as_deref()?;

    let (service, port) = match instance.spec.provider {
        IdentityProvider::Keycloak => (name.to_string(), 80),
        // The API Service, not the webapp: a front end answering proves
        // nothing about the product behind it.
        IdentityProvider::Ferriskey => (
            identity_instance::ferriskey_api_name(name),
            identity_instance::FERRISKEY_API_PORT,
        ),
    };

    Some(format!("http://{service}.{namespace}.svc:{port}"))
}

/// The workloads that carry a product's version, by the names the instance
/// controller gives them.
///
/// Keycloak is one Deployment; FerrisKey is two, and the upgrade is only over
/// when both have moved. This used to assume Keycloak's shape, so for a
/// FerrisKey instance it looked for a Deployment that does not exist, found
/// nothing, and reported "not ready" for ever: the upgrade never finished,
/// nothing reported an outcome, and the console showed an upgrade in progress
/// long after the cluster had finished it.
fn versioned_workloads(instance: &IdentityInstance) -> Vec<(String, &'static str)> {
    let name = instance.metadata.name.clone().unwrap_or_default();

    match instance.spec.provider {
        IdentityProvider::Keycloak => vec![(name, "keycloak")],
        IdentityProvider::Ferriskey => vec![
            (format!("{name}-api"), "ferriskey-api"),
            (format!("{name}-webapp"), "ferriskey-webapp"),
        ],
    }
}

async fn deployment_ready_for_version(
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

    for (deployment_name, container_name) in versioned_workloads(instance) {
        let deployment = deployments
            .get_opt(&deployment_name)
            .await
            .map_err(|error| OperatorError::Kube {
                message: error.to_string(),
            })?;

        let Some(deployment) = deployment else {
            return Ok(false);
        };

        if !workload_ready_on(&deployment, container_name, target_version) {
            return Ok(false);
        }
    }

    Ok(true)
}

fn workload_ready_on(deployment: &Deployment, container_name: &str, target_version: &str) -> bool {
    let has_target_version = deployment
        .spec
        .as_ref()
        .and_then(|spec| spec.template.spec.as_ref())
        .map(|pod_spec| {
            pod_spec.containers.iter().any(|container| {
                container.name == container_name
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

    has_target_version
        && observed_generation >= generation
        && ready_replicas >= desired_replicas
        && available_replicas >= desired_replicas
}

#[cfg(test)]
mod versioned_workload_naming {
    use super::{versioned_workloads, workload_ready_on};
    use aether_crds::v1alpha::identity_instance::{IdentityInstance, IdentityProvider};
    use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec, DeploymentStatus};
    use k8s_openapi::api::core::v1::{Container, PodSpec, PodTemplateSpec};
    use kube::api::ObjectMeta;

    /// Built through the fixture the other tests here use, so the shape of a
    /// spec lives in one place.
    fn instance(provider: IdentityProvider) -> IdentityInstance {
        let mut instance = super::tests::instance_with_provider(provider);
        instance.metadata.name = Some("auth".to_string());
        instance
    }

    /// The bug this exists for: FerrisKey runs two deployments under names
    /// that are not the instance's, so looking for Keycloak's shape found
    /// nothing and reported "not ready" for ever.
    #[test]
    fn ferriskey_carries_its_version_in_two_workloads() {
        let workloads = versioned_workloads(&instance(IdentityProvider::Ferriskey));

        assert_eq!(
            workloads,
            vec![
                ("auth-api".to_string(), "ferriskey-api"),
                ("auth-webapp".to_string(), "ferriskey-webapp"),
            ]
        );
    }

    #[test]
    fn keycloak_carries_its_version_in_one() {
        let workloads = versioned_workloads(&instance(IdentityProvider::Keycloak));

        assert_eq!(workloads, vec![("auth".to_string(), "keycloak")]);
    }

    fn deployment(image: &str, container: &str, ready: i32) -> Deployment {
        Deployment {
            metadata: ObjectMeta {
                generation: Some(2),
                ..Default::default()
            },
            spec: Some(DeploymentSpec {
                template: PodTemplateSpec {
                    spec: Some(PodSpec {
                        containers: vec![Container {
                            name: container.to_string(),
                            image: Some(image.to_string()),
                            ..Default::default()
                        }],
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ..Default::default()
            }),
            status: Some(DeploymentStatus {
                observed_generation: Some(2),
                ready_replicas: Some(ready),
                available_replicas: Some(ready),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn a_workload_on_the_target_version_and_serving_is_ready() {
        let deployment = deployment("ghcr.io/x/ferriskey-api:0.6.0", "ferriskey-api", 1);

        assert!(workload_ready_on(&deployment, "ferriskey-api", "0.6.0"));
    }

    /// Still on the version it came from. The rollout has not reached it yet.
    #[test]
    fn a_workload_left_on_the_old_version_is_not_ready() {
        let deployment = deployment("ghcr.io/x/ferriskey-api:0.5.0", "ferriskey-api", 1);

        assert!(!workload_ready_on(&deployment, "ferriskey-api", "0.6.0"));
    }

    /// On the right version and answering to nobody.
    #[test]
    fn a_workload_with_no_ready_replica_is_not_ready() {
        let deployment = deployment("ghcr.io/x/ferriskey-api:0.6.0", "ferriskey-api", 0);

        assert!(!workload_ready_on(&deployment, "ferriskey-api", "0.6.0"));
    }

    /// A version that is a prefix of another must not pass for it: 0.6.0 is
    /// not 0.6.0-rc1, and `ends_with` is what keeps them apart.
    #[test]
    fn a_neighbouring_tag_is_not_the_target() {
        let deployment = deployment("ghcr.io/x/ferriskey-api:0.6.0-rc1", "ferriskey-api", 1);

        assert!(!workload_ready_on(&deployment, "ferriskey-api", "0.6.0"));
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use k8s_openapi::chrono::Duration as ChronoDuration;

    use super::*;

    fn status_started(elapsed_ago: ChronoDuration) -> IdentityInstanceUpgradeStatus {
        IdentityInstanceUpgradeStatus {
            started_at: Some(Time(Utc::now() - elapsed_ago)),
            ..Default::default()
        }
    }

    pub(super) fn instance_with_provider(provider: IdentityProvider) -> IdentityInstance {
        use aether_crds::common::types::ResourceRequirements;
        use aether_crds::v1alpha::identity_instance::{
            DatabaseConfig, DatabaseMode, IdentityInstanceSpec, ManagedClusterConfig,
            ManagedClusterStorage,
        };
        use kube::core::ObjectMeta;

        IdentityInstance {
            metadata: ObjectMeta {
                name: Some("instance-1".to_string()),
                namespace: Some("aether-instances".to_string()),
                ..Default::default()
            },
            spec: IdentityInstanceSpec {
                organisation_id: "org-1".to_string(),
                provider,
                version: "25.0.0".to_string(),
                // A hostname that resolves nowhere: the point of probing the in-cluster
                // Service instead of the public endpoint is that this must not matter.
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
                ferriskey: None,
                ingress: None,
                allowed_cidrs: None,
            },
            status: None,
        }
    }

    #[test]
    fn keycloak_is_probed_on_its_own_service_over_plain_http_in_cluster() {
        let instance = instance_with_provider(IdentityProvider::Keycloak);

        assert_eq!(
            instance_health_probe_url(&instance).as_deref(),
            Some("http://instance-1.aether-instances.svc:80")
        );
    }

    #[test]
    fn ferriskey_is_probed_on_its_api_service_not_the_webapp() {
        let instance = instance_with_provider(IdentityProvider::Ferriskey);

        assert_eq!(
            instance_health_probe_url(&instance).as_deref(),
            Some("http://instance-1-api.aether-instances.svc:3333")
        );
    }

    #[test]
    fn an_instance_missing_a_namespace_has_no_probe_target() {
        let mut instance = instance_with_provider(IdentityProvider::Keycloak);
        instance.metadata.namespace = None;

        assert_eq!(instance_health_probe_url(&instance), None);
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

    #[test]
    fn a_deployment_that_is_not_ready_is_not_healthy_no_matter_what_the_probe_says() {
        let health = evaluate_runtime_health(false, Some(true), None, Duration::from_secs(60));

        assert_eq!(health, RuntimeHealth::NotReady);
    }

    #[test]
    fn an_upgrade_that_comes_up_but_refuses_traffic_is_not_a_success() {
        let ready_since = Time(Utc::now() - ChronoDuration::minutes(10));

        let health = evaluate_runtime_health(
            true,
            Some(false),
            Some(&ready_since),
            Duration::from_secs(3 * 60),
        );

        assert_eq!(health, RuntimeHealth::Unresponsive);
    }

    #[test]
    fn a_pod_that_just_turned_ready_and_has_not_answered_yet_gets_a_grace_period() {
        let health = evaluate_runtime_health(true, Some(false), None, Duration::from_secs(3 * 60));

        assert_eq!(health, RuntimeHealth::StillWarmingUp);
    }

    #[test]
    fn a_pod_ready_moments_ago_that_has_not_answered_yet_is_still_warming_up() {
        let ready_since = Time(Utc::now() - ChronoDuration::seconds(5));

        let health = evaluate_runtime_health(
            true,
            Some(false),
            Some(&ready_since),
            Duration::from_secs(3 * 60),
        );

        assert_eq!(health, RuntimeHealth::StillWarmingUp);
    }

    #[test]
    fn a_ready_deployment_that_answers_is_healthy_regardless_of_how_long_it_has_been_ready() {
        let ready_since = Time(Utc::now() - ChronoDuration::hours(2));

        let health = evaluate_runtime_health(
            true,
            Some(true),
            Some(&ready_since),
            Duration::from_secs(3 * 60),
        );

        assert_eq!(health, RuntimeHealth::Healthy);
    }

    #[test]
    fn a_ready_deployment_with_no_probe_target_falls_back_to_readiness_alone() {
        let ready_since = Time(Utc::now() - ChronoDuration::hours(2));

        let health =
            evaluate_runtime_health(true, None, Some(&ready_since), Duration::from_secs(3 * 60));

        assert_eq!(
            health,
            RuntimeHealth::Healthy,
            "an unbuildable probe target must never manufacture a rollback on its own"
        );
    }

    #[test]
    fn not_ready_and_ready_but_unresponsive_produce_different_failure_reasons() {
        let not_ready = FailureReason::NotReady.describe("26.0.0");
        let unresponsive = FailureReason::Unresponsive.describe("26.0.0");

        assert_ne!(not_ready, unresponsive);
        assert!(not_ready.contains("did not become ready"));
        assert!(unresponsive.contains("did not answer a health check"));
    }

    async fn probe_with_listener(
        respond: impl FnOnce(std::net::TcpStream) + Send + 'static,
    ) -> bool {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            respond(stream);
        });

        let probe = HttpHealthCheck::new();
        let healthy = probe.probe(&format!("http://{addr}/")).await;
        handle.join().unwrap();
        healthy
    }

    #[tokio::test]
    async fn a_server_answering_with_success_is_healthy() {
        let healthy = probe_with_listener(|mut stream| {
            let mut buffer = [0u8; 1024];
            let _ = stream.read(&mut buffer);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n");
        })
        .await;

        assert!(healthy);
    }

    #[tokio::test]
    async fn a_server_answering_with_a_server_error_is_not_healthy() {
        let healthy = probe_with_listener(|mut stream| {
            let mut buffer = [0u8; 1024];
            let _ = stream.read(&mut buffer);
            let _ =
                stream.write_all(b"HTTP/1.1 503 Service Unavailable\r\ncontent-length: 0\r\n\r\n");
        })
        .await;

        assert!(!healthy);
    }

    #[tokio::test]
    async fn a_port_nothing_is_listening_on_is_not_healthy() {
        // Bind and immediately drop, so the port is refusing connections.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let probe = HttpHealthCheck::new();
        let healthy = probe.probe(&format!("http://{addr}/")).await;

        assert!(!healthy);
    }

    fn upgrade_at(phase: Option<Phase>) -> IdentityInstanceUpgradeStatus {
        IdentityInstanceUpgradeStatus {
            phase,
            ..Default::default()
        }
    }

    #[test]
    fn a_rollback_records_a_distinct_outcome_from_a_plain_failure() {
        let rolled_back = IdentityInstanceUpgradeStatus {
            outcome: Some(UpgradeOutcome::RolledBack),
            ..upgrade_at(Some(Phase::Failed))
        };
        let failed = IdentityInstanceUpgradeStatus {
            outcome: Some(UpgradeOutcome::Failed),
            ..upgrade_at(Some(Phase::Failed))
        };

        assert_ne!(rolled_back.outcome, failed.outcome);
    }
}
