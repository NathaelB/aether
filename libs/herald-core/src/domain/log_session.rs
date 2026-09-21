//! Following one deployment's pods and posting what they say, in batches,
//! until somebody stops watching or the ceiling is reached.
//!
//! Nothing here is written down. Lines exist in a batch for at most
//! [`FLUSH_INTERVAL`] and are gone once the request that carried them
//! returns: the control plane promised never to keep a customer's logs, and a
//! copy left behind in the data plane would break that promise just as
//! thoroughly.

use std::sync::Arc;
use std::time::Duration;

use tokio::time::{Instant, interval, sleep_until};
use tracing::{debug, info, warn};

use crate::domain::entities::logs::{Ending, LogLine, LogStreamRequest};
use crate::domain::ports::{ControlPlaneRepository, LogPushOutcome, PodLogSource};

/// The most lines one request carries.
///
/// A busy instance produces thousands a second, and a request per line would
/// spend more time in HTTP headers than in logs. Bounded rather than
/// unbounded because the batch is also the memory a session holds.
///
/// `pub(crate)`: [`crate::domain::log_shipping`]'s continuous reader reuses
/// this rather than inventing its own number, per #294.
pub(crate) const MAX_BATCH_LINES: usize = 500;

/// How long lines may accumulate before being sent anyway.
///
/// The other half of batching: on a quiet instance the size limit is never
/// reached, and without this the first line would wait for the second one
/// that never comes.
pub(crate) const FLUSH_INTERVAL: Duration = Duration::from_millis(500);

/// How long a quiet session waits before saying it is still there.
///
/// An instance with nothing to say produces no batch, and a control plane
/// that hears nothing has no way to tell that from a data plane that has
/// gone. This is the difference, and it is cheap: one small request every ten
/// seconds, only while there is nothing else to send.
///
/// It pays for itself twice. The answer also says whether anybody is still
/// reading, so a session following a quiet instance after its reader left now
/// stops within ten seconds instead of running to the ceiling below.
const KEEPALIVE: Duration = Duration::from_secs(10);

/// The longest one session may run.
///
/// A ceiling is not optional. The control plane accepts a batch for a session
/// nobody is reading and discards it, so a reader who closed the page leaves
/// no trace Herald can observe -- without a ceiling that session would follow
/// the pods and post every half second for as long as the process lives.
/// Fifteen minutes is longer than anyone watches a log screen in one sitting,
/// and reaching it says so rather than going quiet, so a reader who is still
/// there gets another session without noticing.
const MAX_SESSION: Duration = Duration::from_secs(15 * 60);

/// How many pushes may fail in a row before the session is given up on.
///
/// A blip should not end a session somebody is watching; a control plane that
/// has been refusing for this long is not coming back inside the session's
/// remaining life.
const MAX_CONSECUTIVE_FAILURES: u32 = 3;

/// Follows a deployment's pods until there is nothing more to send.
///
/// Always ends by telling the control plane the session is done, so a reader
/// is never left watching a screen that will not change again -- including
/// when the pods could not be read at all, which ends the session immediately
/// and sends no line pretending to be one.
pub async fn run_log_session<CP, PL>(
    control_plane: Arc<CP>,
    pod_logs: Arc<PL>,
    request: LogStreamRequest,
) where
    CP: ControlPlaneRepository,
    PL: PodLogSource,
{
    let deadline = Instant::now() + MAX_SESSION;

    let mut lines = match pod_logs.follow(&request).await {
        Ok(lines) => lines,
        Err(err) => {
            warn!(
                %err,
                session_id = %request.session_id,
                deployment_id = %request.deployment_id,
                "could not read the deployment's pods; ending the session"
            );
            finish(&control_plane, &request, Vec::new(), Ending::Unreadable).await;
            return;
        }
    };

    let mut batch: Vec<LogLine> = Vec::new();
    let mut flush = interval(FLUSH_INTERVAL);
    let mut contact = Contact::now();

    loop {
        tokio::select! {
            received = lines.recv() => match received {
                Some(line) => {
                    batch.push(line);
                    if batch.len() >= MAX_BATCH_LINES
                        && !send(&control_plane, &request, &mut batch, &mut contact).await
                    {
                        return;
                    }
                }
                // The pods have nothing more to give.
                None => break,
            },
            _ = flush.tick() => {
                // An empty batch is sent too, once the line has been quiet
                // long enough: it is what tells the control plane the
                // instance is silent rather than this data plane.
                if (!batch.is_empty() || contact.last.elapsed() >= KEEPALIVE)
                    && !send(&control_plane, &request, &mut batch, &mut contact).await
                {
                    return;
                }
            }
            _ = sleep_until(deadline) => {
                info!(
                    session_id = %request.session_id,
                    "log session reached its ceiling; ending it"
                );
                break;
            }
        }
    }

    finish(&control_plane, &request, batch, Ending::Finished).await;
}

/// What a session knows about its own line to the control plane.
struct Contact {
    /// When it last got an answer, keepalives included. What decides whether
    /// a quiet instance is due one.
    last: Instant,
    /// Pushes refused in a row.
    failures: u32,
}

impl Contact {
    fn now() -> Self {
        Self {
            last: Instant::now(),
            failures: 0,
        }
    }
}

/// Sends one batch. `false` means this session is over and nothing further
/// should be sent for it.
async fn send<CP>(
    control_plane: &Arc<CP>,
    request: &LogStreamRequest,
    batch: &mut Vec<LogLine>,
    contact: &mut Contact,
) -> bool
where
    CP: ControlPlaneRepository,
{
    let lines = std::mem::take(batch);

    match control_plane.push_log_lines(request, lines, None).await {
        Ok(LogPushOutcome::Relayed) => {
            contact.last = Instant::now();
            contact.failures = 0;
            true
        }
        // Not a failure: the reader closed the page, and there was no way to
        // know that before sending. There is also nobody left to tell the
        // session is done.
        Ok(LogPushOutcome::SessionGone) => {
            debug!(
                session_id = %request.session_id,
                "nobody is reading this session any more; stopping"
            );
            false
        }
        Err(err) => {
            contact.failures += 1;
            warn!(
                %err,
                session_id = %request.session_id,
                attempt = contact.failures,
                "failed to send a batch of log lines"
            );
            // Not counted as contact: a refused push says nothing about
            // whether the reader is still there, so the next quiet tick tries
            // again rather than waiting another ten seconds.
            contact.failures < MAX_CONSECUTIVE_FAILURES
        }
    }
}

/// The last thing every session does: say it is finished, and why.
///
/// Best-effort, and deliberately not retried. If this does not land the
/// reader's stream ends when the control plane's own session does, which is
/// the same outcome one round trip later -- with the reason lost, which is
/// the only thing worth the attempt.
async fn finish<CP>(
    control_plane: &Arc<CP>,
    request: &LogStreamRequest,
    batch: Vec<LogLine>,
    ending: Ending,
) where
    CP: ControlPlaneRepository,
{
    if let Err(err) = control_plane
        .push_log_lines(request, batch, Some(ending))
        .await
    {
        warn!(
            %err,
            session_id = %request.session_id,
            "failed to close the log session"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::dataplane::DataPlaneId;
    use crate::domain::entities::deployment::{DeploymentId, DeploymentKind};
    use crate::domain::entities::logs::{LogSessionId, OrganisationId};
    use crate::domain::error::HeraldError;
    use crate::domain::ports::{MockControlPlaneRepository, MockPodLogSource};
    use chrono::Utc;
    use std::sync::Mutex as StdMutex;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    fn request() -> LogStreamRequest {
        LogStreamRequest {
            deployment_id: DeploymentId::new(Uuid::nil().to_string()),
            dataplane_id: DataPlaneId::new(Uuid::nil().to_string()),
            organisation_id: OrganisationId::new(Uuid::nil().to_string()),
            namespace: "aether-acme".to_string(),
            kind: DeploymentKind::Ferriskey,
            session_id: LogSessionId(Uuid::nil()),
            since_minutes: 5,
        }
    }

    fn line(message: &str) -> LogLine {
        LogLine {
            at: Utc::now(),
            source: "ferriskey-api".to_string(),
            message: message.to_string(),
        }
    }

    /// Everything one run posted: the lines in each batch, and the ending
    /// that batch carried, if it was the last one.
    type Sent = Arc<StdMutex<Vec<(Vec<LogLine>, Option<Ending>)>>>;

    fn recording_control_plane(
        outcomes: Vec<Result<LogPushOutcome, HeraldError>>,
    ) -> (Arc<MockControlPlaneRepository>, Sent) {
        let sent: Sent = Arc::new(StdMutex::new(Vec::new()));
        let recorder = Arc::clone(&sent);
        let outcomes = Arc::new(StdMutex::new(std::collections::VecDeque::from(outcomes)));

        let mut control_plane = MockControlPlaneRepository::new();
        control_plane
            .expect_push_log_lines()
            .returning(move |_, lines, ending| {
                recorder.lock().expect("the recorder").push((lines, ending));
                let outcome = outcomes
                    .lock()
                    .expect("the outcomes")
                    .pop_front()
                    .unwrap_or(Ok(LogPushOutcome::Relayed));
                Box::pin(async move { outcome })
            });

        (Arc::new(control_plane), sent)
    }

    /// A source whose lines are already decided, closed as soon as they run
    /// out.
    ///
    /// Fed from a task rather than up front, like the real adapter: a bounded
    /// channel is what pushes back on a busy instance, and filling it before
    /// anybody is reading would just deadlock.
    fn source_of(lines: Vec<LogLine>) -> Arc<MockPodLogSource> {
        let lines = Arc::new(StdMutex::new(Some(lines)));
        let mut source = MockPodLogSource::new();
        source.expect_follow().returning(move |_| {
            let lines = lines.lock().expect("the lines").take().unwrap_or_default();
            Box::pin(async move {
                let (sender, receiver) = mpsc::channel(64);
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

        Arc::new(source)
    }

    fn all_lines(sent: &Sent) -> Vec<String> {
        sent.lock()
            .expect("the recorder")
            .iter()
            .flat_map(|(lines, _)| lines.iter().map(|line| line.message.clone()))
            .collect()
    }

    #[tokio::test]
    async fn every_line_reaches_the_control_plane_and_the_session_says_it_is_done() {
        let (control_plane, sent) = recording_control_plane(Vec::new());
        let source = source_of(vec![line("one"), line("two"), line("three")]);

        run_log_session(control_plane, source, request()).await;

        assert_eq!(all_lines(&sent), vec!["one", "two", "three"]);
        assert_eq!(
            sent.lock()
                .expect("the recorder")
                .last()
                .expect("a batch")
                .1,
            Some(Ending::Finished),
            "the last request must say the session is over, and why"
        );
    }

    /// Lines go up in batches, not one request each: a busy instance produces
    /// thousands a second, and a request per line is a request per line.
    #[tokio::test]
    async fn lines_are_batched_rather_than_sent_one_at_a_time() {
        let (control_plane, sent) = recording_control_plane(Vec::new());
        let source = source_of((0..50).map(|n| line(&format!("line {n}"))).collect());

        run_log_session(control_plane, source, request()).await;

        let batches = sent.lock().expect("the recorder").len();
        assert!(
            batches < 50,
            "50 lines went up in {batches} requests, which is not batching"
        );
        assert_eq!(all_lines(&sent).len(), 50, "and nothing was dropped");
    }

    /// The acceptance criterion. Pods that cannot be read end the session
    /// immediately rather than leaving somebody watching an empty screen, and
    /// nothing is sent that could be mistaken for something the instance said.
    #[tokio::test]
    async fn pods_that_cannot_be_read_end_the_session_without_inventing_a_line() {
        let (control_plane, sent) = recording_control_plane(Vec::new());

        let mut source = MockPodLogSource::new();
        source.expect_follow().returning(|_| {
            Box::pin(async {
                Err(HeraldError::Internal {
                    message: "no pods in aether-acme".to_string(),
                })
            })
        });

        run_log_session(control_plane, Arc::new(source), request()).await;

        let sent = sent.lock().expect("the recorder");
        assert_eq!(sent.len(), 1, "exactly one request, and it is the last one");
        assert_eq!(
            sent[0].1,
            Some(Ending::Unreadable),
            "and it must say the pods were the problem, not the instance"
        );
        assert!(
            sent[0].0.is_empty(),
            "a session that read nothing must send nothing: {:?}",
            sent[0].0
        );
    }

    /// A reader who closed the page leaves nothing to send to. Carrying on
    /// would follow the pods and post every half second for nobody.
    #[tokio::test]
    async fn a_session_nobody_is_reading_stops_being_sent_to() {
        let (control_plane, sent) = recording_control_plane(vec![Ok(LogPushOutcome::SessionGone)]);
        let source = source_of((0..2_000).map(|n| line(&format!("line {n}"))).collect());

        run_log_session(control_plane, source, request()).await;

        let sent = sent.lock().expect("the recorder");
        assert_eq!(
            sent.len(),
            1,
            "nothing more may be sent once the session is gone, got {} requests",
            sent.len()
        );
        assert_eq!(
            sent[0].1, None,
            "and there is nobody left to tell the session is over"
        );
    }

    /// A control plane that keeps refusing ends the session rather than
    /// retrying for its whole ceiling.
    #[tokio::test]
    async fn a_control_plane_that_keeps_refusing_ends_the_session() {
        let failure = || {
            Err(HeraldError::ControlPlane {
                message: "unavailable".to_string(),
            })
        };
        let (control_plane, sent) =
            recording_control_plane(vec![failure(), failure(), failure(), failure()]);
        let source = source_of((0..2_000).map(|n| line(&format!("line {n}"))).collect());

        run_log_session(control_plane, source, request()).await;

        assert_eq!(
            sent.lock().expect("the recorder").len(),
            MAX_CONSECUTIVE_FAILURES as usize
        );
    }

    /// A session must end on its own. Nothing tells Herald that the reader
    /// closed the page, so without the ceiling a quiet instance would be
    /// followed, and polled, for as long as the process lives.
    ///
    /// Time is paused, so the fifteen minutes pass as soon as everything is
    /// idle rather than in fifteen real minutes.
    #[tokio::test(start_paused = true)]
    async fn a_session_that_nothing_ends_stops_at_its_ceiling() {
        let (control_plane, sent) = recording_control_plane(Vec::new());

        let started = Instant::now();
        run_log_session(control_plane, an_instance_with_nothing_to_say(), request()).await;

        assert!(
            started.elapsed() >= MAX_SESSION,
            "the session ended before its ceiling"
        );
        assert_eq!(
            sent.lock()
                .expect("the recorder")
                .last()
                .expect("a batch")
                .1,
            Some(Ending::Finished),
            "and it ended by saying so"
        );
    }

    /// A source that holds the session open and never says another word: the
    /// sender is kept alive, so the stream never ends on its own.
    fn an_instance_with_nothing_to_say() -> Arc<MockPodLogSource> {
        let held: Arc<StdMutex<Vec<mpsc::Sender<LogLine>>>> = Arc::new(StdMutex::new(Vec::new()));
        let mut source = MockPodLogSource::new();
        source.expect_follow().returning(move |_| {
            let held = Arc::clone(&held);
            Box::pin(async move {
                let (sender, receiver) = mpsc::channel(8);
                held.lock().expect("the senders").push(sender);
                Ok(receiver)
            })
        });

        Arc::new(source)
    }

    /// The point of the whole change. An instance nobody is hitting produces
    /// no batch, and a control plane that hears nothing at all cannot tell
    /// that from a data plane that has gone -- so it says so on its own.
    #[tokio::test(start_paused = true)]
    async fn a_quiet_instance_still_has_the_data_plane_saying_it_is_there() {
        let (control_plane, sent) = recording_control_plane(Vec::new());
        let session = tokio::spawn(run_log_session(
            control_plane,
            an_instance_with_nothing_to_say(),
            request(),
        ));

        tokio::time::sleep(KEEPALIVE * 3 + Duration::from_secs(1)).await;

        let batches = sent.lock().expect("the recorder").clone();
        session.abort();

        assert!(
            batches.len() >= 3,
            "three keepalives were due, {} were sent",
            batches.len()
        );
        assert!(
            batches
                .iter()
                .all(|(lines, ending)| lines.is_empty() && ending.is_none()),
            "a keepalive carries no line and does not end the session: {batches:?}"
        );
    }

    /// The second thing the keepalive buys. Nothing tells Herald that a
    /// reader closed the page; on a quiet instance there was no batch to be
    /// told through, so the session used to run its full ceiling for nobody.
    #[tokio::test(start_paused = true)]
    async fn a_quiet_session_learns_its_reader_left_without_waiting_for_the_ceiling() {
        let (control_plane, sent) = recording_control_plane(vec![Ok(LogPushOutcome::SessionGone)]);

        let started = Instant::now();
        run_log_session(control_plane, an_instance_with_nothing_to_say(), request()).await;

        assert!(
            started.elapsed() < MAX_SESSION,
            "it waited for the ceiling anyway"
        );
        assert_eq!(
            sent.lock().expect("the recorder").len(),
            1,
            "and nothing was sent after being told nobody is reading"
        );
    }

    /// One failure is a blip, not the end of a session somebody is watching.
    #[tokio::test]
    async fn a_single_failed_batch_does_not_end_the_session() {
        let (control_plane, sent) = recording_control_plane(vec![Err(HeraldError::ControlPlane {
            message: "unavailable".to_string(),
        })]);
        let source = source_of((0..1_200).map(|n| line(&format!("line {n}"))).collect());

        run_log_session(control_plane, source, request()).await;

        let sent = sent.lock().expect("the recorder");
        assert!(sent.len() > 1, "the session continued past one failure");
        assert_eq!(
            sent.last().expect("a batch").1,
            Some(Ending::Finished),
            "and ended cleanly"
        );
    }
}
