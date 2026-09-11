use crate::domain::entities::action::{AckFailure, AckOutcome, Action, ActionEvent, ActionId};
use crate::domain::entities::dataplane::DataPlaneId;
use crate::domain::entities::deployment::{Deployment, DeploymentId};
use crate::domain::entities::logs::{LogLine, LogStreamRequest};
use crate::domain::entities::outcome::DeploymentOutcomeReport;
use crate::domain::entities::usage::{CounterSample, UsagePoint, UsageTarget};
use crate::domain::error::HeraldError;
use std::future::Future;
use tokio::sync::mpsc::Receiver;

pub trait HeraldService: Send + Sync {
    fn sync_all_deployments(&self) -> impl Future<Output = Result<(), HeraldError>> + Send;
    fn process_deployment(
        &self,
        deployment_id: &DeploymentId,
    ) -> impl Future<Output = Result<(), HeraldError>> + Send;

    /// Reads every instance this Herald owns, folds the readings into
    /// one-minute buckets, and carries the retained window upward.
    ///
    /// Runs on its own tick rather than inside the sync cycle: the aggregation
    /// only works while consecutive readings stay inside one bucket, and the
    /// sync interval is a knob an operator is free to set to five minutes.
    fn collect_usage(&self) -> impl Future<Output = Result<(), HeraldError>> + Send;
}

#[cfg_attr(test, mockall::automock)]
pub trait ControlPlaneRepository: Send + Sync {
    fn list_deployments(
        &self,
        dp_id: &DataPlaneId,
    ) -> impl Future<Output = Result<Vec<Deployment>, HeraldError>> + Send;

    fn claim_actions(
        &self,
        dp_id: &DataPlaneId,
        deployment_id: &DeploymentId,
    ) -> impl Future<Output = Result<Vec<Action>, HeraldError>> + Send;

    /// Acknowledges the outcome of previously-claimed actions. At-least-once
    /// delivery relies on this happening *after* a successful publish: if the
    /// ack is lost, the lease expires and the action is republished, which is
    /// why consumers deduplicate on `action_id`.
    fn ack_actions(
        &self,
        dp_id: &DataPlaneId,
        deployment_id: &DeploymentId,
        published: Vec<ActionId>,
        failed: Vec<AckFailure>,
    ) -> impl Future<Output = Result<AckOutcome, HeraldError>> + Send;

    /// Reports that this data plane is alive.
    ///
    /// Sent once per sync cycle. Without it the control plane cannot tell a
    /// data plane that is idle from one that is gone, and keeps placing new
    /// deployments on a cluster that will never claim them.
    fn send_heartbeat(
        &self,
        dp_id: &DataPlaneId,
    ) -> impl Future<Output = Result<(), HeraldError>> + Send;

    /// Carries an outcome another data plane component observed.
    ///
    /// Herald is a courier here, not an author: it did not see the thing being
    /// reported, it just holds the credentials to say it.
    fn report_outcome(
        &self,
        dp_id: &DataPlaneId,
        report: &DeploymentOutcomeReport,
    ) -> impl Future<Output = Result<(), HeraldError>> + Send;

    /// Carries already-aggregated buckets for one deployment.
    ///
    /// Buckets only: the raw events never leave the data plane, and the
    /// control plane has no endpoint that would take them. The write is
    /// idempotent on `(deployment, metric, bucket)`, which is what lets Herald
    /// send the same window every cycle instead of tracking what landed.
    fn report_usage(
        &self,
        deployment_id: &DeploymentId,
        points: &[UsagePoint],
    ) -> impl Future<Output = Result<(), HeraldError>> + Send;

    /// Sends one batch of log lines for an open session, `done` marking the
    /// last request that session will ever make.
    ///
    /// Takes the lines by value: they are relayed and forgotten, and nothing
    /// keeps a copy to hand out twice.
    fn push_log_lines(
        &self,
        request: &LogStreamRequest,
        lines: Vec<LogLine>,
        done: bool,
    ) -> impl Future<Output = Result<LogPushOutcome, HeraldError>> + Send;
}

/// What the control plane made of a batch.
///
/// `SessionGone` is not a failure. It is how a reader closing the page reaches
/// a data plane that had no way to know: the batch was accepted and thrown
/// away, and there is nothing left to send to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogPushOutcome {
    Relayed,
    SessionGone,
}

/// The lines a deployment's pods are producing.
///
/// The receiver ends when the pods have nothing more to give, and dropping it
/// is how a session says it has stopped listening -- which is what stops the
/// reads inside the cluster too.
#[cfg_attr(test, mockall::automock)]
pub trait PodLogSource: Send + Sync {
    fn follow(
        &self,
        request: &LogStreamRequest,
    ) -> impl Future<Output = Result<Receiver<LogLine>, HeraldError>> + Send;
}

/// Where one product's usage numbers actually come from.
///
/// `Ok(None)` and `Err(_)` are deliberately different answers. `None` says the
/// product exposes nothing Herald can count, which is a standing fact about
/// the product; `Err` says this instance could not be read, which is a fact
/// about right now. Neither is a zero, and the collector turns neither into
/// one -- it simply never hears about the minute.
#[cfg_attr(test, mockall::automock)]
pub trait UsageSource: Send + Sync {
    fn sample(
        &self,
        target: &UsageTarget,
    ) -> impl Future<Output = Result<Option<CounterSample>, HeraldError>> + Send;
}

/// The outcomes waiting to be carried to the control plane.
#[cfg_attr(test, mockall::automock)]
pub trait OutcomeInboxRepository: Send + Sync {
    /// Takes everything queued, up to `limit`.
    ///
    /// Drained on the sync cycle rather than consumed continuously: outcomes
    /// are rare, and a second long-lived consumer would be a second thing that
    /// can silently stop. `limit` keeps one cycle bounded when a backlog has
    /// built up.
    fn drain(
        &self,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<DeploymentOutcomeReport>, HeraldError>> + Send;
}

#[cfg_attr(test, mockall::automock)]
pub trait MessageBusRepository: Send + Sync {
    fn publish(&self, event: ActionEvent) -> impl Future<Output = Result<(), HeraldError>> + Send;
}
