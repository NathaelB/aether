use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// What Genesis observed happening to a deployment, on its way back to the
/// control plane.
///
/// Carries no data plane id on purpose. Herald addresses the report to the data
/// plane it is configured for, which is the one identity in this path that is
/// authenticated -- a data plane id travelling in the payload would be an
/// unverified claim sitting next to a verified one, and sooner or later
/// something would trust the wrong one.
///
/// Published to the broker rather than sent to the control plane directly, and
/// that is the point: Genesis holds AMQP credentials and Herald holds control
/// plane credentials, and this keeps it that way. Reporting directly would
/// mean a second component learning how to authenticate against the control
/// plane, and duplicating the token refresh that already lives in Herald.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentOutcomeReport {
    pub deployment_id: Uuid,
    /// The vocabulary the control plane parses. A string rather than an enum
    /// because it crosses a process boundary: an outcome this Genesis does not
    /// know about must not stop an older one deserialising the ones it does.
    pub outcome: String,
}

impl DeploymentOutcomeReport {
    /// The resources are gone.
    ///
    /// Reported by the component that removed them, which is what makes it the
    /// one outcome a data plane can state without inferring anything.
    pub fn deleted(deployment_id: Uuid) -> Self {
        Self {
            deployment_id,
            outcome: "deleted".to_string(),
        }
    }
}
