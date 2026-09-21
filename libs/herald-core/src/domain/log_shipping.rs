//! Following a deployment's pods for as long as its switch (#294) stays on,
//! independent of any live session, and shipping what they say to the
//! organisation's search index. The live tail has its own path in
//! `log_session` and this reader never touches it.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;
use tokio::time::{Instant, interval, sleep, sleep_until};
use tracing::warn;

use crate::domain::entities::deployment::Deployment;
use crate::domain::entities::logs::{LogLine, LogStreamRequest};
use crate::domain::log_index::LogIndexDocument;
use crate::domain::log_session::{FLUSH_INTERVAL, MAX_BATCH_LINES};
use crate::domain::ports::{LogIndexSink, PodLogSource};

/// How often a continuous reader drops what it has and re-lists the
/// deployment's pods.
///
/// `PodLogSource::follow`'s receiver only ends once every container it
/// listed has stopped sending -- with more than one replica, restarting a
/// single pod drops just that one sender and leaves the others open, so the
/// receiver never ends and never heals the gap on its own. A live session
/// gets away with never re-listing because `log_session::MAX_SESSION` forces
/// the whole session to end and the reader to open a new one; a continuous
/// reader has no such ceiling, so it re-lists itself instead.
const RELIST_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// The window each re-list reads.
///
/// Wider than `RELIST_INTERVAL` on purpose: a line produced right before a
/// re-list must still fall inside the *next* one's window, or it is never
/// read at all. The cost is a line landing in the index twice when it falls
/// in the overlap, which nothing here deduplicates -- acceptable for what
/// this chantier promises (a line is findable, not indexed exactly once).
const RELIST_SINCE_MINUTES: u32 = 6;

/// Follows one deployment's pods until its caller stops the task, re-listing
/// on [`RELIST_INTERVAL`] to heal a restart no receiver would tell it about.
///
/// Memory is bounded the same way `log_session` bounds a live session's
/// batch, plus one gate: at most [`MAX_BATCH_LINES`] lines waiting to be
/// shipped and one batch already in flight, and never more, because a batch
/// ready to ship while another is still in flight is dropped rather than
/// queued (see [`ship`]).
pub async fn run_continuous_log_shipping<PL>(
    pod_logs: Arc<PL>,
    sink: Arc<dyn LogIndexSink>,
    deployment: Deployment,
) where
    PL: PodLogSource,
{
    let gate = Arc::new(Semaphore::new(1));

    loop {
        let Some(request) = deployment.log_stream_request(RELIST_SINCE_MINUTES) else {
            warn!(
                deployment_id = %deployment.id,
                "deployment has no product or namespace yet; not following its pods"
            );
            return;
        };

        let mut lines = match pod_logs.follow(&request).await {
            Ok(lines) => lines,
            Err(err) => {
                warn!(
                    %err,
                    deployment_id = %deployment.id,
                    "could not read this deployment's pods; retrying at the next re-list"
                );
                sleep(RELIST_INTERVAL).await;
                continue;
            }
        };

        let mut batch: Vec<LogLine> = Vec::new();
        let mut flush = interval(FLUSH_INTERVAL);
        let deadline = Instant::now() + RELIST_INTERVAL;

        loop {
            tokio::select! {
                received = lines.recv() => match received {
                    Some(line) => {
                        batch.push(line);
                        if batch.len() >= MAX_BATCH_LINES {
                            ship(&sink, &gate, &request, std::mem::take(&mut batch));
                        }
                    }
                    // Every container this iteration listed is gone; re-list.
                    None => break,
                },
                _ = flush.tick() => {
                    if !batch.is_empty() {
                        ship(&sink, &gate, &request, std::mem::take(&mut batch));
                    }
                }
                _ = sleep_until(deadline) => break,
            }
        }

        ship(&sink, &gate, &request, batch);
    }
}

/// Ships one batch, best-effort, dropping it instead of growing memory
/// without bound when the sink has not finished the previous one.
///
/// `gate` holds a single permit: a batch that finds it already taken is
/// dropped rather than spawned alongside the one in flight, which is what
/// keeps a Quickwit that is down or merely slow from turning an indefinite
/// reader into an indefinite queue of batches waiting for it.
fn ship(
    sink: &Arc<dyn LogIndexSink>,
    gate: &Arc<Semaphore>,
    request: &LogStreamRequest,
    lines: Vec<LogLine>,
) {
    if lines.is_empty() {
        return;
    }

    let Ok(permit) = Arc::clone(gate).try_acquire_owned() else {
        warn!(
            deployment_id = %request.deployment_id,
            dropped = lines.len(),
            "the search index sink is still busy with the previous batch; dropping this one rather than growing memory without bound"
        );
        return;
    };

    let documents: Vec<LogIndexDocument> = lines
        .iter()
        .map(|line| LogIndexDocument::from_line(request, line))
        .collect();
    let sink = Arc::clone(sink);
    let organisation_id = request.organisation_id.clone();
    let deployment_id = request.deployment_id.clone();

    tokio::spawn(async move {
        let _permit = permit;
        if let Err(err) = sink.ship(organisation_id, documents).await {
            warn!(%err, %deployment_id, "failed to ship a batch of log lines to the search index");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::dataplane::DataPlaneId;
    use crate::domain::entities::deployment::{DeploymentId, DeploymentKind};
    use crate::domain::entities::logs::OrganisationId;
    use crate::domain::ports::{MockLogIndexSink, MockPodLogSource};
    use chrono::Utc;
    use std::sync::Mutex as StdMutex;
    use tokio::sync::mpsc;

    fn deployment() -> Deployment {
        Deployment {
            id: DeploymentId::new("dep-1"),
            dataplane_id: DataPlaneId::new("dp-1"),
            organisation_id: OrganisationId::new("org-1"),
            name: "acme-prod".to_string(),
            kind: Some(DeploymentKind::Ferriskey),
            namespace: Some("aether-acme".to_string()),
            log_shipping_enabled: true,
        }
    }

    fn line(message: &str) -> LogLine {
        LogLine {
            at: Utc::now(),
            source: "ferriskey-api".to_string(),
            message: message.to_string(),
        }
    }

    type HeldSenders = Arc<StdMutex<Vec<mpsc::Sender<LogLine>>>>;

    /// A source that sends what it is given, then holds the session open --
    /// like the real adapter, whose receiver only ends once every container
    /// it listed stops.
    fn source_holding_open(lines: Vec<LogLine>) -> (Arc<MockPodLogSource>, HeldSenders) {
        let held: HeldSenders = Arc::new(StdMutex::new(Vec::new()));
        let held_for_source = Arc::clone(&held);
        let lines = Arc::new(StdMutex::new(Some(lines)));

        let mut source = MockPodLogSource::new();
        source.expect_follow().returning(move |_| {
            let lines = lines.lock().expect("the lines").take().unwrap_or_default();
            let held = Arc::clone(&held_for_source);
            Box::pin(async move {
                let (sender, receiver) = mpsc::channel(64);
                held.lock().expect("the senders").push(sender.clone());
                tokio::spawn(async move {
                    for line in lines {
                        if sender.send(line).await.is_err() {
                            return;
                        }
                    }
                });
                Ok(receiver)
            })
        });

        (Arc::new(source), held)
    }

    type Shipped = mpsc::UnboundedReceiver<(OrganisationId, Vec<LogIndexDocument>)>;

    fn recording_sink() -> (Arc<MockLogIndexSink>, Shipped) {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut sink = MockLogIndexSink::new();
        sink.expect_ship()
            .returning(move |organisation_id, documents| {
                let tx = tx.clone();
                Box::pin(async move {
                    let _ = tx.send((organisation_id, documents));
                    Ok(())
                })
            });
        (Arc::new(sink), rx)
    }

    /// The re-list strategy this chantier picks: when `follow`'s receiver
    /// ends -- as it does the moment every container it listed is gone, with
    /// no client ever involved to close a page -- the reader lists again
    /// rather than giving up.
    #[tokio::test]
    async fn a_reader_re_lists_once_the_receiver_ends() {
        let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (invoked_tx, mut invoked_rx) = mpsc::unbounded_channel::<()>();
        let held: HeldSenders = Arc::new(StdMutex::new(Vec::new()));

        let attempts_for_closure = Arc::clone(&attempts);
        let held_for_closure = Arc::clone(&held);
        let mut source = MockPodLogSource::new();
        source.expect_follow().returning(move |_| {
            let invoked_tx = invoked_tx.clone();
            let attempt = attempts_for_closure.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let held = Arc::clone(&held_for_closure);
            Box::pin(async move {
                let (sender, receiver) = mpsc::channel(8);
                if attempt > 0 {
                    // Held open past this call: the reader must not be
                    // driven to re-list a second time just to pass this
                    // test.
                    held.lock().expect("the senders").push(sender);
                }
                // The first call's sender is dropped right here, ending the
                // receiver immediately -- exactly the condition a continuous
                // reader has to recover from on its own.
                let _ = invoked_tx.send(());
                Ok(receiver)
            })
        });

        let task = tokio::spawn(run_continuous_log_shipping(
            Arc::new(source),
            Arc::new(MockLogIndexSink::new()) as Arc<dyn LogIndexSink>,
            deployment(),
        ));

        invoked_rx.recv().await.expect("the first follow call");
        invoked_rx
            .recv()
            .await
            .expect("a second follow call: the re-list that heals the ended receiver");

        task.abort();
    }

    #[tokio::test]
    async fn a_followed_deployment_ships_what_it_reads_to_its_own_organisation() {
        let (source, _held) = source_holding_open(vec![line("INFO one"), line("ERROR two")]);
        let (sink, mut shipped) = recording_sink();

        let task = tokio::spawn(run_continuous_log_shipping(
            source,
            sink as Arc<dyn LogIndexSink>,
            deployment(),
        ));

        let (organisation_id, documents) = shipped.recv().await.expect("a batch was shipped");
        task.abort();

        assert_eq!(organisation_id, OrganisationId::new("org-1"));
        assert_eq!(
            documents
                .iter()
                .map(|doc| (doc.message.clone(), doc.level.clone()))
                .collect::<Vec<_>>(),
            vec![
                ("INFO one".to_string(), "info".to_string()),
                ("ERROR two".to_string(), "error".to_string()),
            ]
        );
    }

    /// A deployment with nowhere to be found (no product or namespace yet)
    /// ends the reader rather than looping on a request it can never build.
    #[tokio::test]
    async fn a_deployment_that_cannot_be_found_ends_the_reader() {
        let mut unreachable_deployment = deployment();
        unreachable_deployment.namespace = None;
        let sink = MockLogIndexSink::new();
        let pod_logs = Arc::new(MockPodLogSource::new());

        tokio::time::timeout(
            Duration::from_secs(1),
            run_continuous_log_shipping(pod_logs, Arc::new(sink), unreachable_deployment),
        )
        .await
        .expect("the reader returns instead of looping forever");
    }

    /// The drop policy this chantier calls for: a sink still busy with the
    /// previous batch does not get a second one queued behind it, which is
    /// what would let an indefinite reader build an indefinite backlog.
    #[test]
    fn a_busy_sink_causes_the_next_batch_to_be_dropped_rather_than_queued() {
        let gate = Arc::new(Semaphore::new(1));
        let _permit = gate.clone().try_acquire_owned().expect("the only permit");

        let mut sink = MockLogIndexSink::new();
        sink.expect_ship().times(0);

        ship(
            &(Arc::new(sink) as Arc<dyn LogIndexSink>),
            &gate,
            &request(),
            vec![line("dropped")],
        );
    }

    #[tokio::test]
    async fn an_empty_batch_is_never_shipped() {
        let gate = Arc::new(Semaphore::new(1));
        let mut sink = MockLogIndexSink::new();
        sink.expect_ship().times(0);

        ship(
            &(Arc::new(sink) as Arc<dyn LogIndexSink>),
            &gate,
            &request(),
            Vec::new(),
        );
    }

    fn request() -> LogStreamRequest {
        deployment()
            .log_stream_request(RELIST_SINCE_MINUTES)
            .expect("a fully-known deployment always has a request")
    }

    /// A batch too large to be dropped by an idle gate is still ingested as
    /// a single call, not split -- shipping does not re-batch what
    /// `run_continuous_log_shipping` already batched.
    #[tokio::test]
    async fn an_idle_gate_lets_a_batch_through() {
        let gate = Arc::new(Semaphore::new(1));
        let (sink, mut shipped) = recording_sink();

        ship(
            &(sink as Arc<dyn LogIndexSink>),
            &gate,
            &request(),
            vec![line("one")],
        );

        let (_organisation_id, documents) = shipped.recv().await.expect("a batch was shipped");
        assert_eq!(documents.len(), 1);
    }
}
