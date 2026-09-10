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

    /// The version observed running, when the observation says one.
    ///
    /// Skipped when absent so a control plane built before this field reads
    /// the report unchanged. Adding a field is safe in that direction; the
    /// reverse is not, which is why the consumer went first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
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
            version: None,
        }
    }

    /// The instance is serving, on this version.
    ///
    /// The version is what tells an upgrade that landed from one that has not
    /// started: an instance answers on its old version until the rollout
    /// replaces it, and this watcher resyncs long before that.
    pub fn running(deployment_id: Uuid, version: impl Into<String>) -> Self {
        Self {
            deployment_id,
            outcome: "running".to_string(),
            version: Some(version.into()),
        }
    }

    pub fn failed(deployment_id: Uuid) -> Self {
        Self {
            deployment_id,
            outcome: "failed".to_string(),
            version: None,
        }
    }
}
