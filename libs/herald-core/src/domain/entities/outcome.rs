use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// What a data plane component observed happening to a deployment.
///
/// Herald does not produce these -- Genesis does, and Herald carries them to
/// the control plane. That split is deliberate: Genesis is the component that
/// touches Kubernetes and therefore knows, and Herald is the component that
/// holds control plane credentials. Neither gains what the other has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentOutcomeReport {
    pub deployment_id: Uuid,
    pub dataplane_id: Uuid,
    pub outcome: String,
}
