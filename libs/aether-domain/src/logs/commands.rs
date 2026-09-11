use crate::{
    deployments::DeploymentId,
    logs::{LogLine, LogSessionId, LogWindow},
    organisation::OrganisationId,
};

pub struct ReadLogsCommand {
    pub organisation_id: OrganisationId,
    pub deployment_id: DeploymentId,
    pub window: LogWindow,
    /// Chosen by the caller rather than minted here, so the session can be
    /// registered with the relay before the data plane is told about it. The
    /// other way round leaves a window in which a batch arrives for a session
    /// that does not exist yet, and the data plane reads that as the reader
    /// having left.
    pub session_id: LogSessionId,
}

/// A batch on its way in from a data plane.
///
/// Batched rather than one line at a time because a busy instance produces
/// thousands a second, and a request per line would spend the whole budget on
/// HTTP rather than on logs.
pub struct PushLogLinesCommand {
    pub session_id: LogSessionId,
    pub deployment_id: DeploymentId,
    pub lines: Vec<LogLine>,
    /// The data plane has nothing further to send for this session.
    pub done: bool,
}
