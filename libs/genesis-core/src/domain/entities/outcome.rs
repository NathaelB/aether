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

    /// How long a drill (#185) took, start to finish. Absent for every
    /// outcome that is not a drill result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u64>,

    /// Why a drill failed, for the audit log a failed drill writes. Absent
    /// for every outcome that is not a failed drill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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
            duration_seconds: None,
            reason: None,
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
            duration_seconds: None,
            reason: None,
        }
    }

    pub fn failed(deployment_id: Uuid) -> Self {
        Self {
            deployment_id,
            outcome: "failed".to_string(),
            version: None,
            duration_seconds: None,
            reason: None,
        }
    }

    /// A drill (#185) restored this deployment's own archive end to end and
    /// confirmed the database answers a query. The duration is the restore
    /// time objective measured rather than claimed.
    pub fn drill_succeeded(deployment_id: Uuid, duration_seconds: u64) -> Self {
        Self {
            deployment_id,
            outcome: "drill_succeeded".to_string(),
            version: None,
            duration_seconds: Some(duration_seconds),
            reason: None,
        }
    }

    /// A drill (#185) failed. The duration is carried when the failure
    /// happened after the drill started timing, and omitted when it failed
    /// before there was anything to time.
    pub fn drill_failed(
        deployment_id: Uuid,
        reason: impl Into<String>,
        duration_seconds: Option<u64>,
    ) -> Self {
        Self {
            deployment_id,
            outcome: "drill_failed".to_string(),
            version: None,
            duration_seconds,
            reason: Some(reason.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEPLOYMENT: Uuid = Uuid::from_u128(1);

    #[test]
    fn a_succeeded_drill_carries_its_duration_and_no_reason() {
        let report = DeploymentOutcomeReport::drill_succeeded(DEPLOYMENT, 212);

        assert_eq!(report.outcome, "drill_succeeded");
        assert_eq!(report.duration_seconds, Some(212));
        assert_eq!(report.reason, None);
        assert_eq!(report.version, None);
    }

    #[test]
    fn a_failed_drill_carries_its_reason_and_an_optional_duration() {
        let report =
            DeploymentOutcomeReport::drill_failed(DEPLOYMENT, "corrupted archive", Some(64));

        assert_eq!(report.outcome, "drill_failed");
        assert_eq!(report.reason.as_deref(), Some("corrupted archive"));
        assert_eq!(report.duration_seconds, Some(64));
    }

    /// A drill that failed before there was anything to time -- an instance
    /// that never even applied -- still reports, just with nothing to say
    /// about duration.
    #[test]
    fn a_failed_drill_may_carry_no_duration() {
        let report = DeploymentOutcomeReport::drill_failed(DEPLOYMENT, "could not apply", None);

        assert_eq!(report.duration_seconds, None);
    }

    /// Every existing outcome stays untouched by the two new fields: a
    /// control plane reading a `running`/`failed`/`deleted` report sees
    /// exactly what it always has, once the optional fields are skipped.
    #[test]
    fn ordinary_outcomes_carry_neither_new_field() {
        for report in [
            DeploymentOutcomeReport::deleted(DEPLOYMENT),
            DeploymentOutcomeReport::running(DEPLOYMENT, "26.0.1"),
            DeploymentOutcomeReport::failed(DEPLOYMENT),
        ] {
            assert_eq!(report.duration_seconds, None);
            assert_eq!(report.reason, None);
        }
    }
}
