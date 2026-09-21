use std::collections::HashSet;
use std::future::Future;

use aether_auth::Identity;

use crate::{
    CoreError,
    logs::commands::{PushLogLinesCommand, ReadLogsCommand},
    logs::{
        LogLine, LogSearchFilter, LogSearchResult, LogSession, LogSessionId, LogSignatureCount,
        Relayed, SessionEnd,
    },
    organisation::OrganisationId,
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
    /// An empty batch is not a no-op: it is a data plane saying it is still
    /// following and simply has nothing to report, and the reader is told so.
    /// Without that, an instance producing nothing and a data plane that has
    /// gone away look the same from here.
    ///
    /// Answers whether anybody was there to receive it. A session nobody is
    /// reading any more is not a failure, it is how a session ends when
    /// somebody closes the page; but the data plane has no other way to learn
    /// that, and without being told it keeps sending until its own ceiling.
    fn push(
        &self,
        session_id: LogSessionId,
        lines: Vec<LogLine>,
    ) -> impl Future<Output = Result<bool, CoreError>> + Send;

    /// Tells the reader why there will be nothing more, then forgets the
    /// session. Idempotent: a data plane that says it twice, or says it after
    /// the reader left, is behaving normally.
    fn end(
        &self,
        session_id: LogSessionId,
        end: SessionEnd,
    ) -> impl Future<Output = Result<(), CoreError>> + Send;

    /// Forgets a session without telling anybody, for the one case where
    /// there is nobody to tell: a read that was refused after the session was
    /// registered but before the data plane was asked for anything.
    fn close(&self, session_id: LogSessionId)
    -> impl Future<Output = Result<(), CoreError>> + Send;
}

/// The receiving end of a session.
///
/// Ends when the data plane says it is done, when the session is closed, or
/// when whatever is behind it goes away. There is no way to rewind: a line
/// that has been handed out is gone from here.
pub trait LogStream: Send {
    /// The next thing that happened, which is not always a line. `None` means
    /// the stream is over with nothing left to say -- the case a reader
    /// should never see, because [`Relayed::Ended`] comes first.
    fn next(&mut self) -> impl Future<Output = Option<Relayed>> + Send;
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

    /// Takes a batch from a data plane and forwards it. Answers whether
    /// anybody is still reading the session.
    fn push_log_lines(
        &self,
        identity: Identity,
        command: PushLogLinesCommand,
    ) -> impl Future<Output = Result<bool, CoreError>> + Send;
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

/// Where a search actually runs.
///
/// Its own port, separate from `herald-core::infrastructure::logs::quickwit`'s
/// ingest sink: shipping and searching the index are different capabilities
/// held by different processes, and a control-plane crate must not depend on
/// the data-plane crate just to get one adapter's shape.
///
/// `organisation_id` is its own argument rather than a field on
/// [`LogSearchFilter`], so the one thing that picks which index is even
/// reachable can never be something a filter value carries instead -- it can
/// only come from wherever the caller already checked it against `identity`.
#[cfg_attr(test, mockall::automock)]
pub trait LogSearchIndex: Send + Sync {
    fn search(
        &self,
        organisation_id: OrganisationId,
        filter: LogSearchFilter,
    ) -> impl Future<Output = Result<LogSearchResult, CoreError>> + Send;

    /// A terms aggregation on `fingerprint`, scoped by the same filter a
    /// search would use -- one entry per distinct signature the window
    /// holds, up to [`crate::logs::MAX_SIGNATURES`]. Carries no notion of
    /// "new"; that is [`crate::logs::service::LogSearchServiceImpl::group`]'s
    /// job, once it has a baseline to compare against.
    fn group(
        &self,
        organisation_id: OrganisationId,
        filter: LogSearchFilter,
    ) -> impl Future<Output = Result<Vec<LogSignatureCount>, CoreError>> + Send;

    /// The same aggregation as [`Self::group`], read back as just the set of
    /// fingerprints present -- what a baseline window needs, without paying
    /// for a representative message per signature nobody will show.
    fn distinct_fingerprints(
        &self,
        organisation_id: OrganisationId,
        filter: LogSearchFilter,
    ) -> impl Future<Output = Result<HashSet<String>, CoreError>> + Send;
}
