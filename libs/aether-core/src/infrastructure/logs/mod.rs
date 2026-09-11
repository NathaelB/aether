use std::{collections::HashMap, sync::Arc};

use tokio::sync::{Mutex, mpsc};
use tracing::debug;

use aether_domain::{
    CoreError,
    logs::{
        LogLine, LogSession, LogSessionId,
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
    sessions: Arc<Mutex<HashMap<LogSessionId, mpsc::Sender<LogLine>>>>,
}

impl InProcessLogRelay {
    pub fn new() -> Self {
        Self::default()
    }
}

/// The receiving end of an in-process session.
pub struct ChannelLogStream(mpsc::Receiver<LogLine>);

impl LogStream for ChannelLogStream {
    async fn next_line(&mut self) -> Option<LogLine> {
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

    async fn push(&self, session_id: LogSessionId, lines: Vec<LogLine>) -> Result<(), CoreError> {
        let sender = {
            let sessions = self.sessions.lock().await;
            sessions.get(&session_id).cloned()
        };

        // No session is not a failure. The reader closed the page, and the
        // data plane had no way to know that before it sent.
        let Some(sender) = sender else {
            debug!(session = %session_id, "lines arrived for a session nobody is reading");
            return Ok(());
        };

        for line in lines {
            if sender.send(line).await.is_err() {
                self.close(session_id).await?;
                return Ok(());
            }
        }

        Ok(())
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

        relay
            .push(session.id, vec![line("hello")])
            .await
            .expect("pushed");

        assert_eq!(stream.next_line().await.expect("a line").message, "hello");
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

        assert_eq!(stream.next_line().await.expect("a line").message, "first");
        assert_eq!(stream.next_line().await.expect("a line").message, "second");
    }

    /// The reader closed the page. The data plane is still sending, because
    /// it had no way to know. That is ordinary, not an error.
    #[tokio::test]
    async fn pushing_to_a_session_nobody_reads_is_not_an_error() {
        let relay = InProcessLogRelay::new();

        relay
            .push(LogSessionId(Uuid::new_v4()), vec![line("nobody home")])
            .await
            .expect("not an error");
    }

    #[tokio::test]
    async fn closing_a_session_ends_the_stream() {
        let relay = InProcessLogRelay::new();
        let session = session();
        let mut stream = relay.open(session.clone()).await.expect("opened");

        relay.close(session.id).await.expect("closed");

        assert!(stream.next_line().await.is_none());
    }

    /// Said twice, or said after the reader left. Both are normal.
    #[tokio::test]
    async fn closing_twice_is_harmless() {
        let relay = InProcessLogRelay::new();
        let session = session();
        relay.open(session.clone()).await.expect("opened");

        relay.close(session.id).await.expect("closed");
        relay.close(session.id).await.expect("closed again");
    }

    /// A session that ended while the data plane was still sending must not
    /// leave the sender behind, or the map grows for the life of the process.
    #[tokio::test]
    async fn a_reader_that_left_is_forgotten() {
        let relay = InProcessLogRelay::new();
        let session = session();
        let stream = relay.open(session.clone()).await.expect("opened");
        drop(stream);

        relay
            .push(session.id, vec![line("into the void")])
            .await
            .expect("not an error");

        assert!(relay.sessions.lock().await.is_empty());
    }
}
