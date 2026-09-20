use crate::domain::entities::action::{AckFailure, AckOutcome, Action, ActionEvent, ActionId};
use crate::domain::entities::archive::Archive;
use crate::domain::entities::certificate::ReceivedCertificate;
use crate::domain::entities::dataplane::DataPlaneId;
use crate::domain::entities::deployment::{Deployment, DeploymentId};
use crate::domain::entities::logs::{Ending, LogLine, LogStreamRequest, OrganisationId};
use crate::domain::entities::outcome::DeploymentOutcomeReport;
use crate::domain::entities::usage::{CounterSample, UsagePoint, UsageTarget};
use crate::domain::error::HeraldError;
use crate::domain::log_index::LogIndexDocument;
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

    /// Claims every pending action across `deployment_ids` in one call.
    ///
    /// `deployment_ids` is expected to already be narrowed to this Herald's
    /// shard (see [`crate::domain::entities::shard::ShardConfig`]): the shard
    /// travels with the request as the concrete set of deployments it covers,
    /// rather than as a `(shard_index, shard_count)` pair the control plane
    /// would have to hash itself -- which is a second, independent notion of
    /// shard, and one already exists for `list_deployments` that disagrees
    /// with this crate's own hash.
    fn claim_actions(
        &self,
        dp_id: &DataPlaneId,
        deployment_ids: &[DeploymentId],
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

    /// Reports that this data plane is alive, and carries back whatever the
    /// control plane says changed since the last cycle.
    ///
    /// Sent once per sync cycle. Without it the control plane cannot tell a
    /// data plane that is idle from one that is gone, and keeps placing new
    /// deployments on a cluster that will never claim them.
    ///
    /// `known_certificate_fingerprint` is what this cluster's own Gateway TLS
    /// Secret currently holds, if this Herald tracks one at all -- reported
    /// so the control plane can skip resending a certificate this data plane
    /// already has.
    fn send_heartbeat(
        &self,
        dp_id: &DataPlaneId,
        known_certificate_fingerprint: Option<String>,
    ) -> impl Future<Output = Result<HeartbeatOutcome, HeraldError>> + Send;

    /// Carries an outcome another data plane component observed.
    ///
    /// Herald is a courier here, not an author: it did not see the thing being
    /// reported, it just holds the credentials to say it.
    fn report_outcome(
        &self,
        dp_id: &DataPlaneId,
        report: &DeploymentOutcomeReport,
    ) -> impl Future<Output = Result<(), HeraldError>> + Send;

    /// Carries what a drill (#185) proved, or did not.
    ///
    /// Its own endpoint rather than `report_outcome`'s: a drill result must
    /// never be mistaken for the deployment lifecycle transition that
    /// endpoint drives, and routing it separately is what makes that
    /// impossible rather than merely unlikely.
    fn report_drill_outcome(
        &self,
        dp_id: &DataPlaneId,
        report: &DeploymentOutcomeReport,
    ) -> impl Future<Output = Result<(), HeraldError>> + Send;

    /// Carries what the cluster observed about one archive.
    ///
    /// Idempotent on the object key at the other end, which is what lets this
    /// send the same archive again rather than track what landed. A failed
    /// attempt goes to the same endpoint: they are the two outcomes of one
    /// attempt, not two kinds of event.
    fn report_archive(
        &self,
        dp_id: &DataPlaneId,
        archive: &Archive,
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

    /// Sends one batch of log lines for an open session, `ending` marking the
    /// last request that session will ever make and saying why.
    ///
    /// No lines and no ending is the keepalive: still following, nothing to
    /// report. It is also how a session following a quiet instance learns its
    /// reader has gone, which otherwise only happens at the ceiling.
    ///
    /// Takes the lines by value: they are relayed and forgotten, and nothing
    /// keeps a copy to hand out twice.
    fn push_log_lines(
        &self,
        request: &LogStreamRequest,
        lines: Vec<LogLine>,
        ending: Option<Ending>,
    ) -> impl Future<Output = Result<LogPushOutcome, HeraldError>> + Send;
}

/// What a heartbeat's response said changed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HeartbeatOutcome {
    /// The certificate this cluster's own Gateway should be serving, present
    /// only when it differs from what `known_certificate_fingerprint`
    /// reported. Absent means exactly that: nothing changed, or this
    /// installation distributes no certificate at all -- either way, there
    /// is nothing new to write.
    pub certificate: Option<ReceivedCertificate>,
}

/// Where this cluster's own Gateway TLS Secret is written.
///
/// One method: writing is all a heartbeat cycle needs, the same way the
/// control plane's own certificate source only ever reads. What this
/// cluster's own Secret currently holds is tracked in memory by whoever
/// calls this rather than read back before every write -- a Herald that just
/// restarted asks for the certificate once more and writes exactly what it
/// already had, which costs one redundant write and nothing else.
///
/// Boxed rather than `impl Future`, unlike every other port here: whether
/// this installation manages its Gateway's certificate at all is a runtime
/// decision, not one [`crate::domain::services::HeraldServiceImpl`] is built
/// fresh for either way -- it holds this as `Option<Arc<dyn
/// GatewayCertificateSink>>`, which return-position `impl Future` cannot
/// name.
#[cfg_attr(test, mockall::automock)]
pub trait GatewayCertificateSink: Send + Sync {
    fn write<'a>(
        &'a self,
        certificate: ReceivedCertificate,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<(), HeraldError>> + Send + 'a>>;
}

/// Ships a batch of a deployment's lines to the organisation's search index
/// (#293's `logs-{organisation_id}`), read by a continuous reader (#294)
/// independent of the live relay -- which never calls this at all.
///
/// Best-effort: a caller logs and swallows an `Err` the same way it already
/// does for the heartbeat and outcome reporting.
///
/// Boxed rather than `impl Future`, like [`GatewayCertificateSink`]: whether
/// this installation ships to an index at all is a runtime decision -- no
/// Quickwit endpoint configured means shipping is simply off, and the live
/// tail is unaffected -- held as `Option<Arc<dyn LogIndexSink>>` rather than
/// [`crate::domain::services::HeraldServiceImpl`] being built fresh either
/// way.
#[cfg_attr(test, mockall::automock)]
pub trait LogIndexSink: Send + Sync {
    fn ship<'a>(
        &'a self,
        organisation_id: OrganisationId,
        documents: Vec<LogIndexDocument>,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<(), HeraldError>> + Send + 'a>>;
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

/// The archives this data plane has taken, as the cluster records them.
///
/// Read rather than subscribed to. An archive is an hourly-to-daily event, and
/// a long-lived watch is a second thing that can silently stop -- which for
/// backups means finding out during a restore. Reports are idempotent, so
/// reading the same window every cycle is cheaper than remembering what was
/// sent.
#[cfg_attr(test, mockall::automock)]
pub trait ArchiveSource: Send + Sync {
    fn finished_archives(&self) -> impl Future<Output = Result<Vec<Archive>, HeraldError>> + Send;
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
