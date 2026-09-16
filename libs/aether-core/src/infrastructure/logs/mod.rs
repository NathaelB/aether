use std::{collections::HashMap, sync::Arc};

use tokio::sync::{Mutex, mpsc};
use tracing::debug;

use aether_domain::{
    CoreError,
    logs::{
        LogLine, LogSession, LogSessionId, Relayed, SessionEnd,
        ports::{LogRelay, LogStream},
    },
};

/// How many batches may wait for a reader before the data plane is slowed
/// down.
///
/// Small on purpose. A reader who cannot keep up is watching a screen, and a
/// long queue would only let the control plane accumulate lines it has
/// promised never to keep.
const BUFFER: usize = 64;

/// Log lines held in this process, for as long as somebody is reading them.
///
/// Nothing here reaches disk. That is the point: the control plane relays
/// lines and forgets them, so it never becomes the keeper of every customer's
/// personal data.
///
/// The consequence is that a session belongs to the replica that opened it. A
/// data plane pushing to a different replica finds no session and is told so.
/// With more than one replica this needs either sticky routing for the push
/// path or a relay that is not in a process; the port exists so that is an
/// adapter and nothing else.
#[derive(Clone, Default)]
pub struct InProcessLogRelay {
    sessions: Arc<Mutex<HashMap<LogSessionId, mpsc::Sender<Relayed>>>>,
}

impl InProcessLogRelay {
    pub fn new() -> Self {
        Self::default()
    }

    /// Hands whatever happened to the reader, in order.
    ///
    /// Answers whether anybody was there. No session is not a failure: the
    /// reader closed the page, and the data plane had no way to know that
    /// before it sent. Saying so is what lets it stop within one batch
    /// instead of at its own ceiling.
    async fn send(
        &self,
        session_id: LogSessionId,
        events: Vec<Relayed>,
    ) -> Result<bool, CoreError> {
        let sender = {
            let sessions = self.sessions.lock().await;
            sessions.get(&session_id).cloned()
        };

        let Some(sender) = sender else {
            debug!(session = %session_id, "a batch arrived for a session nobody is reading");
            return Ok(false);
        };

        for event in events {
            if sender.send(event).await.is_err() {
                self.sessions.lock().await.remove(&session_id);
                return Ok(false);
            }
        }

        Ok(true)
    }
}

/// The receiving end of an in-process session.
pub struct ChannelLogStream(mpsc::Receiver<Relayed>);

impl LogStream for ChannelLogStream {
    async fn next(&mut self) -> Option<Relayed> {
        self.0.recv().await
    }
}

impl LogRelay for InProcessLogRelay {
    type Stream = ChannelLogStream;

    async fn open(&self, session: LogSession) -> Result<ChannelLogStream, CoreError> {
        let (sender, receiver) = mpsc::channel(BUFFER);
        self.sessions.lock().await.insert(session.id, sender);

        Ok(ChannelLogStream(receiver))
    }

    async fn push(&self, session_id: LogSessionId, lines: Vec<LogLine>) -> Result<bool, CoreError> {
        // An empty batch still has something to say, so it is relayed as the
        // one thing it means: the data plane is there and the instance is
        // quiet.
        let relayed = if lines.is_empty() {
            vec![Relayed::StillFollowing]
        } else {
            lines.into_iter().map(Relayed::Line).collect()
        };

        self.send(session_id, relayed).await
    }

    async fn end(&self, session_id: LogSessionId, end: SessionEnd) -> Result<(), CoreError> {
        // Sent before the session is forgotten, so the reader learns why
        // rather than watching the connection close under them.
        self.send(session_id, vec![Relayed::Ended(end)]).await?;
        self.close(session_id).await
    }

    async fn close(&self, session_id: LogSessionId) -> Result<(), CoreError> {
        self.sessions.lock().await.remove(&session_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_domain::{deployments::DeploymentId, logs::LogWindow};
    use chrono::Utc;
    use uuid::Uuid;

    fn session() -> LogSession {
        LogSession {
            id: LogSessionId(Uuid::new_v4()),
            deployment_id: DeploymentId(Uuid::new_v4()),
            window: LogWindow::minutes(5).expect("inside the cap"),
            opened_at: Utc::now(),
        }
    }

    fn line(message: &str) -> LogLine {
        LogLine {
            at: Utc::now(),
            source: "keycloak".to_string(),
            message: message.to_string(),
        }
    }

    #[tokio::test]
    async fn a_pushed_line_reaches_the_reader() {
        let relay = InProcessLogRelay::new();
        let session = session();
        let mut stream = relay.open(session.clone()).await.expect("opened");

        assert!(
            relay
                .push(session.id, vec![line("hello")])
                .await
                .expect("pushed"),
            "somebody is reading"
        );

        assert_eq!(message(&mut stream).await, "hello");
    }

    #[tokio::test]
    async fn lines_arrive_in_the_order_they_were_sent() {
        let relay = InProcessLogRelay::new();
        let session = session();
        let mut stream = relay.open(session.clone()).await.expect("opened");

        relay
            .push(session.id, vec![line("first"), line("second")])
            .await
            .expect("pushed");

        assert_eq!(message(&mut stream).await, "first");
        assert_eq!(message(&mut stream).await, "second");
    }

    /// The whole point of the empty batch: an instance with nothing to say
    /// must reach the reader as a data plane that is still there, not as
    /// nothing at all.
    #[tokio::test]
    async fn an_empty_batch_tells_the_reader_the_data_plane_is_still_there() {
        let relay = InProcessLogRelay::new();
        let session = session();
        let mut stream = relay.open(session.clone()).await.expect("opened");

        assert!(
            relay.push(session.id, vec![]).await.expect("pushed"),
            "somebody is reading"
        );

        assert_eq!(
            stream.next().await.expect("something"),
            Relayed::StillFollowing
        );
    }

    /// The reader closed the page. The data plane is still sending, because
    /// it had no way to know. That is ordinary, not an error, but it has to
    /// be answerable or the data plane keeps sending until its own ceiling.
    #[tokio::test]
    async fn pushing_to_a_session_nobody_reads_says_so_without_failing() {
        let relay = InProcessLogRelay::new();

        let listening = relay
            .push(LogSessionId(Uuid::new_v4()), vec![line("nobody home")])
            .await
            .expect("not an error");

        assert!(!listening);
    }

    /// An empty batch is answered the same way, which is how a data plane
    /// following a quiet instance learns its reader has gone.
    #[tokio::test]
    async fn an_empty_batch_for_a_session_nobody_reads_says_so_too() {
        let relay = InProcessLogRelay::new();

        let listening = relay
            .push(LogSessionId(Uuid::new_v4()), vec![])
            .await
            .expect("not an error");

        assert!(!listening);
    }

    /// The reason arrives before the stream does. A reader who only saw the
    /// close would have to guess between a quiet instance and a broken one.
    #[tokio::test]
    async fn a_session_that_ends_says_why_before_it_closes() {
        let relay = InProcessLogRelay::new();
        let session = session();
        let mut stream = relay.open(session.clone()).await.expect("opened");

        relay
            .end(session.id, SessionEnd::Unreadable)
            .await
            .expect("ended");

        assert_eq!(
            stream.next().await.expect("something"),
            Relayed::Ended(SessionEnd::Unreadable)
        );
        assert!(stream.next().await.is_none(), "and then nothing");
    }

    #[tokio::test]
    async fn closing_a_session_ends_the_stream() {
        let relay = InProcessLogRelay::new();
        let session = session();
        let mut stream = relay.open(session.clone()).await.expect("opened");

        relay.close(session.id).await.expect("closed");

        assert!(stream.next().await.is_none());
    }

    /// Said twice, or said after the reader left. Both are normal.
    #[tokio::test]
    async fn ending_twice_is_harmless() {
        let relay = InProcessLogRelay::new();
        let session = session();
        relay.open(session.clone()).await.expect("opened");

        relay
            .end(session.id, SessionEnd::Finished)
            .await
            .expect("ended");
        relay
            .end(session.id, SessionEnd::Finished)
            .await
            .expect("ended again");
    }

    /// A session that ended while the data plane was still sending must not
    /// leave the sender behind, or the map grows for the life of the process.
    #[tokio::test]
    async fn a_reader_that_left_is_forgotten() {
        let relay = InProcessLogRelay::new();
        let session = session();
        let stream = relay.open(session.clone()).await.expect("opened");
        drop(stream);

        let listening = relay
            .push(session.id, vec![line("into the void")])
            .await
            .expect("not an error");

        assert!(!listening);
        assert!(relay.sessions.lock().await.is_empty());
    }

    async fn message(stream: &mut ChannelLogStream) -> String {
        match stream.next().await.expect("something") {
            Relayed::Line(line) => line.message,
            other => panic!("expected a line, got {other:?}"),
        }
    }
}
