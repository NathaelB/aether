use std::collections::HashMap;
use std::sync::Arc;

use aether_crds::common::types::Phase;
use aether_crds::v1alpha::identity_instance::{IdentityInstance, IdentityInstanceStatus};
use kube::runtime::watcher;
use kube::runtime::watcher::Event;
use kube::{Api, Client};
use tokio_stream::StreamExt;
use tracing::{info, warn};
use uuid::Uuid;

use crate::domain::entities::outcome::DeploymentOutcomeReport;
use crate::domain::error::GenesisError;
use crate::domain::ports::OutcomePublisher;

/// Watches `IdentityInstance` resources and reports what the operator decided.
///
/// The operator is the only component that knows whether a deployment came up:
/// it writes `phase` and `ready` after reconciling a database, a migration, a
/// Deployment and an Ingress. It has no control plane client, and giving it one
/// would put credentials in a third component.
///
/// So Genesis watches instead. It already holds a Kubernetes client and an
/// outcome publisher, which makes it the component that can carry this without
/// gaining anything new -- the same argument that put the outcome path through
/// the broker in the first place.
///
/// Reports only what the operator wrote. `Running` with `ready` is a deployment
/// serving traffic; `Failed` is the operator saying it gave up. Neither is
/// inferred from a timeout, which is why this needs no policy about when to
/// declare a deployment lost.
pub struct IdentityInstanceStatusWatcher {
    client: Client,
    outcomes: Arc<dyn OutcomePublisher>,
}

impl IdentityInstanceStatusWatcher {
    pub fn new(client: Client, outcomes: Arc<dyn OutcomePublisher>) -> Self {
        Self { client, outcomes }
    }

    /// Runs until the watch stream ends, which it does not do on its own.
    pub async fn run(&self) -> Result<(), GenesisError> {
        let api: Api<IdentityInstance> = Api::all(self.client.clone());
        let mut stream = Box::pin(watcher::watcher(api, watcher::Config::default()));

        // What has already been reported, so a resync -- which redelivers every
        // resource -- does not republish the whole cluster every time the watch
        // reconnects. Lost on restart, which costs one duplicate report per
        // deployment; the control plane ignores a report that changes nothing.
        let mut reported: HashMap<Uuid, &'static str> = HashMap::new();

        info!("watching IdentityInstance status");

        while let Some(event) = stream.next().await {
            let instance = match event {
                Ok(Event::Apply(instance)) => instance,
                Ok(Event::InitApply(instance)) => instance,
                // Deletion is already reported by the delete handler, which
                // knows it removed the resource. Reporting it here as well
                // would race with it for no benefit.
                Ok(Event::Delete(_)) | Ok(Event::Init) | Ok(Event::InitDone) => continue,
                Err(err) => {
                    // The watcher reconnects on its own; failing the loop here
                    // would take Genesis down over a dropped connection.
                    warn!(%err, "identity instance watch error");
                    continue;
                }
            };

            let Some(outcome) = outcome_for(instance.status.as_ref()) else {
                continue;
            };

            let Some(deployment_id) = deployment_id_of(instance.metadata.name.as_deref()) else {
                // Something else created an IdentityInstance in this cluster.
                // Not ours to report on.
                continue;
            };

            if reported.get(&deployment_id) == Some(&outcome) {
                continue;
            }

            let report = DeploymentOutcomeReport {
                deployment_id,
                outcome: outcome.to_string(),
            };

            match self.outcomes.publish(report).await {
                Ok(()) => {
                    reported.insert(deployment_id, outcome);
                }
                Err(err) => {
                    // Left unrecorded on purpose, so the next event for this
                    // resource tries again. The watcher resyncs periodically,
                    // so "the next event" is a matter of minutes at worst.
                    warn!(%err, %deployment_id, "failed to publish an outcome");
                }
            }
        }

        Ok(())
    }
}

/// The outcome an operator-written status corresponds to, if any.
///
/// Every other phase is a deployment still on its way, and reporting one would
/// replace a truthful `in_progress` with a different kind of "not yet".
fn outcome_for(status: Option<&IdentityInstanceStatus>) -> Option<&'static str> {
    let status = status?;

    match status.phase {
        // `ready` as well as the phase: `Running` says the resources exist,
        // `ready` says traffic can reach them, and only the pair means the
        // deployment is actually serving.
        Some(Phase::Running) if status.ready => Some("running"),
        Some(Phase::Failed) => Some("failed"),
        _ => None,
    }
}

/// Recovers the deployment id from the resource name.
///
/// Genesis names these `deployment-{uuid}` when it applies them, so the name is
/// the link back to the control plane's row. Anything not matching that shape
/// was created by something else and is not reported on.
fn deployment_id_of(name: Option<&str>) -> Option<Uuid> {
    name?
        .strip_prefix("deployment-")
        .and_then(|id| Uuid::parse_str(id).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(phase: Phase, ready: bool) -> IdentityInstanceStatus {
        IdentityInstanceStatus {
            phase: Some(phase),
            ready,
            ..Default::default()
        }
    }

    #[test]
    fn a_running_and_ready_instance_is_reported_as_running() {
        assert_eq!(
            outcome_for(Some(&status(Phase::Running, true))),
            Some("running")
        );
    }

    /// `Running` says the resources exist; `ready` says traffic reaches them.
    /// Reporting on the phase alone would mark a deployment live while its
    /// ingress is still coming up.
    #[test]
    fn running_without_ready_is_not_reported() {
        assert_eq!(outcome_for(Some(&status(Phase::Running, false))), None);
    }

    /// The operator saying it gave up is an observation, not a timeout, which
    /// is why reporting failure needs no policy about when to declare one.
    #[test]
    fn a_failed_instance_is_reported_however_ready_reads() {
        assert_eq!(
            outcome_for(Some(&status(Phase::Failed, false))),
            Some("failed")
        );
    }

    /// Every intermediate phase is a deployment still on its way. Reporting one
    /// would replace a truthful `in_progress` with a different kind of "not
    /// yet".
    #[test]
    fn phases_on_the_way_are_not_reported() {
        for phase in [
            Phase::Pending,
            Phase::DatabaseProvisioning,
            Phase::Deploying,
            Phase::Updating,
            Phase::Upgrading,
            Phase::Deleting,
        ] {
            assert_eq!(
                outcome_for(Some(&status(phase.clone(), true))),
                None,
                "{phase:?}"
            );
        }
    }

    #[test]
    fn an_instance_with_no_status_yet_is_not_reported() {
        assert_eq!(outcome_for(None), None);
    }

    #[test]
    fn the_deployment_id_is_recovered_from_the_name() {
        let id = uuid::Uuid::new_v4();

        assert_eq!(
            deployment_id_of(Some(&format!("deployment-{id}"))),
            Some(id)
        );
    }

    /// A cluster can hold `IdentityInstance` resources nobody here created --
    /// the local tooling installs one as an example. Reporting on those would
    /// send the control plane an id it has never seen.
    #[test]
    fn an_instance_this_data_plane_did_not_create_is_ignored() {
        assert_eq!(deployment_id_of(Some("cloud-iam-ferriskey")), None);
        assert_eq!(deployment_id_of(Some("deployment-not-a-uuid")), None);
        assert_eq!(deployment_id_of(None), None);
    }
}
