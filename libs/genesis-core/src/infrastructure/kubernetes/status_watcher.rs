use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
/// How long an unchanged status stays quiet before being reported again.
///
/// This is what makes the watcher converge rather than fire once. Publishing a
/// report is not delivering it, and a status that went missing downstream would
/// otherwise stay wrong until something else happened to that deployment.
const REPORT_AGAIN_AFTER: Duration = Duration::from_secs(300);

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

        // What was last reported, and when. Suppresses the republishing of the
        // whole cluster on every resync, without suppressing it for ever.
        //
        // The expiry is the important half. A report can be lost after it is
        // published -- a broker restart, a consumer that cannot read it -- and
        // a watcher that fires once per transition would leave that deployment
        // wrong until something else happened to it. Re-reporting on a slow
        // clock makes the status converge instead: whatever went missing is
        // sent again, and the control plane ignores a report that changes
        // nothing.
        let mut reported: HashMap<Uuid, (&'static str, Option<String>, Instant)> = HashMap::new();

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

            // Read from the spec rather than the status: the spec is what the
            // operator was told to run, and it patches it before the rollout
            // begins, so it moves at the same moment the upgrade does.
            let version = instance.spec.version.clone();

            let previous = reported
                .get(&deployment_id)
                .map(|(last, last_version, at)| (*last, last_version.as_deref(), at.elapsed()));

            if !should_report(previous, outcome, Some(version.as_str())) {
                continue;
            }

            let report = DeploymentOutcomeReport {
                deployment_id,
                outcome: outcome.to_string(),
                version: Some(version.clone()),
            };

            match self.outcomes.publish(report).await {
                Ok(()) => {
                    reported.insert(deployment_id, (outcome, Some(version), Instant::now()));
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

/// What was last published for a deployment, and how long ago.
type LastReport<'a> = (&'a str, Option<&'a str>, Duration);

/// Whether an observation is worth publishing, given what was last published
/// for this deployment and how long ago.
///
/// A change always is. An unchanged one is, once it has gone quiet for long
/// enough -- which is what turns this from a watcher that fires on transitions
/// into one that converges. Publishing a report is not delivering it, and
/// without the second clause a status lost downstream stays wrong until
/// something else happens to that deployment.
///
/// The version counts as part of the observation. An upgrade leaves the
/// outcome at `running` and moves only the version, so comparing outcomes
/// alone would hold the news that it landed for as long as the quiet window.
fn should_report(previous: Option<LastReport<'_>>, outcome: &str, version: Option<&str>) -> bool {
    match previous {
        None => true,
        Some((last, last_version, _)) if last != outcome || last_version != version => true,
        Some((_, _, since)) => since >= REPORT_AGAIN_AFTER,
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
    fn a_deployment_never_reported_on_is_reported() {
        assert!(should_report(None, "running", Some("26.0.0")));
    }

    #[test]
    fn a_changed_outcome_is_reported_immediately() {
        assert!(should_report(
            Some(("running", Some("26.0.0"), Duration::from_secs(1))),
            "failed",
            Some("26.0.0")
        ));
    }

    /// The resync redelivers every resource. Without this, a data plane with a
    /// hundred deployments would republish all hundred every time the watch
    /// reconnects.
    #[test]
    fn an_unchanged_outcome_stays_quiet_for_a_while() {
        assert!(!should_report(
            Some(("running", Some("26.0.0"), Duration::from_secs(1))),
            "running",
            Some("26.0.0")
        ));
    }

    /// The half that makes this converge, and the one this project needed: a
    /// report was published, lost downstream, and the deployment stayed wrong.
    /// Re-reporting on a slow clock repairs that without anyone noticing.
    #[test]
    fn an_unchanged_outcome_is_reported_again_once_it_has_gone_stale() {
        assert!(should_report(
            Some(("running", Some("26.0.0"), REPORT_AGAIN_AFTER)),
            "running",
            Some("26.0.0")
        ));
    }

    /// An upgrade never changes the outcome: the instance was running before
    /// and is running after. Only the version moves, so comparing outcomes
    /// alone would sit on the news for as long as the quiet window.
    #[test]
    fn a_version_that_moved_is_reported_immediately() {
        assert!(should_report(
            Some(("running", Some("26.0.0"), Duration::from_secs(1))),
            "running",
            Some("26.0.1")
        ));
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
