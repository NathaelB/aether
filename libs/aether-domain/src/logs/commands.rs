use crate::{
    deployments::DeploymentId,
    logs::{LogLine, LogSessionId, LogWindow},
    organisation::OrganisationId,
};

pub struct ReadLogsCommand {
    pub organisation_id: OrganisationId,
    pub deployment_id: DeploymentId,
    pub window: LogWindow,
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
