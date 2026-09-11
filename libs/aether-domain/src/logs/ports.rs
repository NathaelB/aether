use std::future::Future;

use aether_auth::Identity;

use crate::{
    CoreError,
    logs::commands::{PushLogLinesCommand, ReadLogsCommand},
    logs::{LogLine, LogSession, LogSessionId},
};

/// Where lines live between arriving from a data plane and reaching whoever
/// asked for them.
///
/// A port rather than a detail, because "in memory, in this process" is a
/// decision with consequences: it ties a session to one replica. Naming it
/// here means the day that stops being acceptable, the replacement is an
/// adapter and nothing else changes.
///
/// Nothing in this trait persists. There is deliberately no method to read a
/// session's lines back after the fact, because there is nowhere to read them
/// from.
pub trait LogRelay: Send + Sync {
    /// The end lines come out of. An associated type rather than a channel,
    /// because naming a channel type here would put the transport in the
    /// domain, which is the one thing this port exists to keep out.
    type Stream: LogStream;

    /// Registers a session and hands back the end that lines come out of.
    fn open(
        &self,
        session: LogSession,
    ) -> impl Future<Output = Result<Self::Stream, CoreError>> + Send;

    /// Pushes a batch towards whoever is holding the stream.
    ///
    /// A session nobody is listening to any more is not an error: the reader
    /// closed the page, and the data plane has no way to have known that yet.
    fn push(
        &self,
        session_id: LogSessionId,
        lines: Vec<LogLine>,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// Ends a session. Idempotent: a data plane that says `done` twice, or
    /// says it after the reader left, is behaving normally.
    fn close(&self, session_id: LogSessionId)
    -> impl Future<Output = Result<(), CoreError>> + Send;
}

/// The receiving end of a session.
///
/// Ends when the data plane says it is done, when the session is closed, or
/// when whatever is behind it goes away. There is no way to rewind: a line
/// that has been handed out is gone from here.
pub trait LogStream: Send {
    fn next_line(&mut self) -> impl Future<Output = Option<LogLine>> + Send;
}

pub trait LogService: Send + Sync {
    /// The end lines come out of, chosen by whatever relay is wired in.
    type Stream: LogStream;

    /// Opens a read, records that it happened, and asks the data plane to
    /// start sending.
    ///
    /// Hands back the stream alongside the session because the caller has to
    /// hold it: dropping it ends the session, which is exactly the lifetime
    /// these lines should have.
    fn read_logs(
        &self,
        identity: Identity,
        command: ReadLogsCommand,
    ) -> impl Future<Output = Result<(LogSession, Self::Stream), CoreError>> + Send;

    /// Takes a batch from a data plane and forwards it.
    fn push_log_lines(
        &self,
        identity: Identity,
        command: PushLogLinesCommand,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;
}

/// Who may read an instance's logs.
///
/// Its own permission rather than a side effect of being able to see the
/// deployment. Logs carry identities, addresses and sometimes tokens, so
/// being allowed to know an instance exists and being allowed to read what
/// its users did are not the same right.
pub trait LogPolicy: Send + Sync {
    fn can_read_logs(
        &self,
        identity: Identity,
        organisation_id: crate::organisation::OrganisationId,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;
}
