use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::dataplane::DataPlaneId;
use super::deployment::{DeploymentId, DeploymentKind};
use crate::domain::error::HeraldError;

/// The action type the control plane records when somebody opens a log view.
///
/// Handled here rather than published onward: every other action is work for
/// another component in this data plane, and this one is work for Herald --
/// it is the only process that holds credentials for both the cluster and the
/// control plane.
pub const LOG_ACTION_TYPE: &str = "deployment.logs";

/// The furthest back a request may reach, matching the control plane's cap.
///
/// Checked again here because a payload is only as trustworthy as the thing
/// that wrote it, and a window Herald does not recognise is a malformed
/// request rather than an invitation to guess.
pub const MAX_WINDOW_MINUTES: u32 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LogSessionId(pub Uuid);

impl std::fmt::Display for LogSessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The organisation a deployment belongs to, carried alongside
/// `deployment_id` so Herald can address the organisation's own search index
/// (`logs-{organisation_id}`, frozen by #293) without asking the control
/// plane a second time.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrganisationId(pub String);

impl OrganisationId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for OrganisationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One line on its way to whoever asked for it.
///
/// Held in memory for as long as it takes to batch it, and never written
/// anywhere: a line that reached disk in the data plane would be a copy of a
/// customer's personal data that nobody agreed to keep.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogLine {
    pub at: DateTime<Utc>,
    /// The container it came from, so a customer running two can tell them
    /// apart.
    pub source: String,
    pub message: String,
}

/// Why a session is stopping, said in the last request it makes.
///
/// Sent rather than left to a closed connection: a reader who only sees the
/// stream stop cannot tell an instance that was never readable from one that
/// simply has nothing to say, and will keep waiting for the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ending {
    /// The pods ran out, or the session reached its ceiling. Opening another
    /// one is how a reader keeps watching.
    Finished,
    /// The pods could not be read at all. Opening another would fail the same
    /// way, so the reader is better told than left retrying.
    Unreadable,
}

/// What the control plane asked a data plane to start sending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogStreamRequest {
    pub deployment_id: DeploymentId,
    pub dataplane_id: DataPlaneId,
    pub organisation_id: OrganisationId,
    pub namespace: String,
    pub kind: DeploymentKind,
    pub session_id: LogSessionId,
    pub since_minutes: u32,
}

impl TryFrom<&Value> for LogStreamRequest {
    type Error = HeraldError;

    fn try_from(payload: &Value) -> Result<Self, Self::Error> {
        let uuid = |field: &str| -> Result<Uuid, HeraldError> {
            payload
                .get(field)
                .and_then(Value::as_str)
                .and_then(|raw| Uuid::parse_str(raw).ok())
                .ok_or_else(|| HeraldError::InvalidAction {
                    message: format!("log request has no valid {field}"),
                })
        };

        let text = |field: &str| -> Result<String, HeraldError> {
            payload
                .get(field)
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .ok_or_else(|| HeraldError::InvalidAction {
                    message: format!("log request has no {field}"),
                })
        };

        let kind = match text("kind")?.as_str() {
            "ferriskey" => DeploymentKind::Ferriskey,
            "keycloak" => DeploymentKind::Keycloak,
            other => {
                return Err(HeraldError::InvalidAction {
                    message: format!("log request names an unknown product '{other}'"),
                });
            }
        };

        let since_minutes = payload
            .get("since_minutes")
            .and_then(Value::as_u64)
            .filter(|minutes| *minutes >= 1 && *minutes <= u64::from(MAX_WINDOW_MINUTES))
            .ok_or_else(|| HeraldError::InvalidAction {
                message: format!(
                    "log request needs a since_minutes between 1 and {MAX_WINDOW_MINUTES}"
                ),
            })? as u32;

        Ok(Self {
            deployment_id: DeploymentId::new(uuid("deployment_id")?.to_string()),
            dataplane_id: DataPlaneId::new(uuid("dataplane_id")?.to_string()),
            organisation_id: OrganisationId::new(uuid("organisation_id")?.to_string()),
            namespace: text("namespace")?,
            kind,
            session_id: LogSessionId(uuid("session_id")?),
            since_minutes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn payload() -> Value {
        json!({
            "deployment_id": "22222222-2222-2222-2222-222222222222",
            "dataplane_id": "11111111-1111-1111-1111-111111111111",
            "organisation_id": "55555555-5555-5555-5555-555555555555",
            "namespace": "aether-acme-prod",
            "kind": "ferriskey",
            "session_id": "33333333-3333-3333-3333-333333333333",
            "since_minutes": 15
        })
    }

    #[test]
    fn a_request_is_read_from_the_payload_the_control_plane_writes() {
        let request = LogStreamRequest::try_from(&payload()).expect("a valid request");

        assert_eq!(
            request.deployment_id,
            DeploymentId::new("22222222-2222-2222-2222-222222222222")
        );
        assert_eq!(
            request.organisation_id,
            OrganisationId::new("55555555-5555-5555-5555-555555555555")
        );
        assert_eq!(request.namespace, "aether-acme-prod");
        assert_eq!(request.kind, DeploymentKind::Ferriskey);
        assert_eq!(request.since_minutes, 15);
    }

    /// The control plane caps the window, and Herald checks it again. A cap
    /// enforced in only one place is a cap that disappears the moment
    /// something else writes an action.
    #[test]
    fn a_window_beyond_the_cap_is_refused_rather_than_shortened() {
        let mut payload = payload();
        payload["since_minutes"] = json!(MAX_WINDOW_MINUTES + 1);

        let error = LogStreamRequest::try_from(&payload).expect_err("outside the cap");

        assert!(
            matches!(error, HeraldError::InvalidAction { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn a_window_of_zero_is_refused() {
        let mut payload = payload();
        payload["since_minutes"] = json!(0);

        assert!(LogStreamRequest::try_from(&payload).is_err());
    }

    #[test]
    fn a_payload_naming_an_unknown_product_is_refused() {
        let mut payload = payload();
        payload["kind"] = json!("okta");

        assert!(LogStreamRequest::try_from(&payload).is_err());
    }

    #[test]
    fn a_payload_missing_the_session_is_refused() {
        let mut payload = payload();
        payload["session_id"] = json!("not-a-uuid");

        assert!(LogStreamRequest::try_from(&payload).is_err());
    }

    /// The control plane always has this value at hand
    /// (`AcceptedLogRead.deployment.organisation_id`); a payload without it
    /// is malformed, not an invitation to guess the tenant.
    #[test]
    fn a_payload_missing_the_organisation_is_refused() {
        let mut payload = payload();
        payload["organisation_id"] = json!("not-a-uuid");

        assert!(LogStreamRequest::try_from(&payload).is_err());
    }

    #[test]
    fn a_payload_with_an_empty_namespace_is_refused() {
        let mut payload = payload();
        payload["namespace"] = json!("  ");

        assert!(LogStreamRequest::try_from(&payload).is_err());
    }
}
