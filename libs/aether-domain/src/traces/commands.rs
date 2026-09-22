use crate::{deployments::DeploymentId, logs::LogSearchWindow, organisation::OrganisationId};

/// A search of an organisation's trace index. `deployment_id` narrows to
/// one deployment's spans when given; left out, the search reaches every
/// deployment the organisation's own index holds -- the same shape as
/// `crate::logs::commands::SearchLogsCommand`.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchTracesCommand {
    pub organisation_id: OrganisationId,
    pub deployment_id: Option<DeploymentId>,
    pub window: LogSearchWindow,
    pub service_name: Option<String>,
    pub status_code: Option<String>,
    pub text: Option<String>,
}

/// Every span of one trace, for the waterfall view.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadTraceCommand {
    pub organisation_id: OrganisationId,
    pub trace_id: String,
}
