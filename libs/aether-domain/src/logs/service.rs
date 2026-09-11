use chrono::Utc;
use tracing::info;
use uuid::Uuid;

use aether_auth::Identity;

use crate::{
    CoreError,
    audit::{
        AuditAction, AuditEntry, AuditEntryId, AuditTarget, AuditTargetKind,
        ports::AuditRepository, service::audit_actor,
    },
    deployments::{Deployment, ports::DeploymentRepository},
    logs::{
        LogSession, LogSessionId,
        commands::ReadLogsCommand,
        ports::{LogPolicy, LogRelay},
    },
    user::ports::UserRepository,
};

/// A read that has been allowed, with everything the caller needs to ask the
/// data plane for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedLogRead {
    pub session: LogSession,
    pub deployment: Deployment,
}

pub struct LogServiceImpl<D, A, U, P, R>
where
    D: DeploymentRepository,
    A: AuditRepository,
    U: UserRepository,
    P: LogPolicy,
    R: LogRelay,
{
    deployment_repository: D,
    audit_repository: A,
    user_repository: U,
    policy: P,
    relay: R,
}

impl<D, A, U, P, R> LogServiceImpl<D, A, U, P, R>
where
    D: DeploymentRepository,
    A: AuditRepository,
    U: UserRepository,
    P: LogPolicy,
    R: LogRelay,
{
    pub fn new(
        deployment_repository: D,
        audit_repository: A,
        user_repository: U,
        policy: P,
        relay: R,
    ) -> Self {
        Self {
            deployment_repository,
            audit_repository,
            user_repository,
            policy,
            relay,
        }
    }

    /// Allows a read, and records that it happened.
    ///
    /// The entry is written before a single line moves. A trail that is
    /// appended after the fact is missing exactly the reads that failed
    /// halfway, which are the ones worth looking at.
    pub async fn accept_read(
        &self,
        identity: Identity,
        command: ReadLogsCommand,
    ) -> Result<AcceptedLogRead, CoreError> {
        self.policy
            .can_read_logs(identity.clone(), command.organisation_id)
            .await?;

        let deployment = self
            .deployment_repository
            .get_by_id(command.deployment_id)
            .await?
            .filter(|deployment| deployment.organisation_id == command.organisation_id)
            .ok_or(CoreError::DeploymentNotFound {
                id: command.deployment_id.0,
            })?;

        let actor = audit_actor(&identity, &self.user_repository).await?;
        let now = Utc::now();

        self.audit_repository
            .append(AuditEntry::record(
                AuditEntryId(Uuid::new_v4()),
                command.organisation_id,
                actor,
                AuditAction("deployment.logs.read".to_string()),
                AuditTarget {
                    kind: AuditTargetKind::Deployment,
                    id: deployment.id.0,
                },
                // No change: nothing was altered. How far back the read
                // reached is the part worth keeping, and it names no line.
                None,
                now,
            ))
            .await?;

        let session = LogSession {
            id: LogSessionId(Uuid::new_v4()),
            deployment_id: deployment.id,
            window: command.window,
            opened_at: now,
        };

        info!(
            deployment_id = %deployment.id,
            session = %session.id,
            minutes = command.window.as_minutes(),
            "opening a log session"
        );

        Ok(AcceptedLogRead {
            session,
            deployment,
        })
    }

    /// The relay this service was built with, for a caller that has to hold
    /// the receiving end open.
    pub fn relay(&self) -> &R {
        &self.relay
    }
}

/// The command a data plane is handed so it knows what to send.
pub fn log_request_payload(accepted: &AcceptedLogRead) -> serde_json::Value {
    serde_json::json!({
        "deployment_id": accepted.deployment.id.0,
        "dataplane_id": accepted.deployment.dataplane_id.0,
        "namespace": accepted.deployment.namespace.clone(),
        "kind": accepted.deployment.kind.to_string(),
        "session_id": accepted.session.id.0,
        "since_minutes": accepted.session.window.as_minutes(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        audit::{AuditActor, AuditEntry},
        dataplane::value_objects::{DataPlaneId, DeploymentResources},
        deployments::{
            DeploymentId, DeploymentKind, DeploymentName, DeploymentStatus,
            ports::MockDeploymentRepository,
        },
        logs::{LogLine, LogSessionId, LogWindow},
        organisation::OrganisationId,
        user::{User, UserId},
        version::Version,
    };
    use std::sync::{Arc, Mutex};

    const ORGANISATION: Uuid = Uuid::from_u128(1);
    const DEPLOYMENT: Uuid = Uuid::from_u128(2);
    const USER: Uuid = Uuid::from_u128(3);

    #[derive(Clone, Default)]
    struct SpyAudit(Arc<Mutex<Vec<AuditEntry>>>);

    impl AuditRepository for SpyAudit {
        async fn append(&self, entry: AuditEntry) -> Result<(), CoreError> {
            self.0.lock().expect("not poisoned").push(entry);
            Ok(())
        }

        async fn list_for_organisation(
            &self,
            _organisation_id: crate::organisation::OrganisationId,
            _cursor: Option<crate::audit::AuditCursor>,
            _limit: usize,
        ) -> Result<crate::audit::AuditBatch, CoreError> {
            unreachable!("a log read never lists the trail")
        }
    }

    #[derive(Clone)]
    struct StubUsers;

    impl UserRepository for StubUsers {
        async fn upsert_by_email(&self, _user: &User) -> Result<User, CoreError> {
            unreachable!("a log read never writes a user")
        }

        async fn find_by_sub(&self, _sub: &str) -> Result<Option<User>, CoreError> {
            Ok(Some(User {
                id: UserId(USER),
                email: "someone@example.test".to_string(),
                name: "Someone".to_string(),
                sub: USER.to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }))
        }
    }

    #[derive(Clone, Copy)]
    struct StubPolicy(bool);

    impl LogPolicy for StubPolicy {
        async fn can_read_logs(
            &self,
            _identity: Identity,
            _organisation_id: OrganisationId,
        ) -> Result<(), CoreError> {
            if self.0 {
                Ok(())
            } else {
                Err(CoreError::PermissionDenied {
                    reason: "not allowed to read logs".to_string(),
                })
            }
        }
    }

    #[derive(Clone, Default)]
    struct NoRelay;

    struct NoLines;

    impl crate::logs::ports::LogStream for NoLines {
        async fn next_line(&mut self) -> Option<LogLine> {
            None
        }
    }

    impl LogRelay for NoRelay {
        type Stream = NoLines;

        async fn open(&self, _session: LogSession) -> Result<NoLines, CoreError> {
            Ok(NoLines)
        }

        async fn push(
            &self,
            _session_id: LogSessionId,
            _lines: Vec<LogLine>,
        ) -> Result<(), CoreError> {
            Ok(())
        }

        async fn close(&self, _session_id: LogSessionId) -> Result<(), CoreError> {
            Ok(())
        }
    }

    fn caller() -> Identity {
        Identity::User(aether_auth::User {
            id: USER.to_string(),
            username: "someone".to_string(),
            email: None,
            name: None,
            roles: vec![],
        })
    }

    fn deployment(organisation: Uuid) -> Deployment {
        let at = Utc::now();
        Deployment {
            id: DeploymentId(DEPLOYMENT),
            organisation_id: OrganisationId(organisation),
            dataplane_id: DataPlaneId(Uuid::from_u128(9)),
            name: DeploymentName("auth".to_string()),
            kind: DeploymentKind::Ferriskey,
            version: Version::new(26, 0, 0),
            status: DeploymentStatus::Successful,
            namespace: "tenant-a".to_string(),
            resources: DeploymentResources::DEFAULT,
            created_by: UserId(USER),
            created_at: at,
            updated_at: at,
            deployed_at: None,
            deleted_at: None,
            auto_upgrade: Default::default(),
            maintenance_window: None,
        }
    }

    fn repository(found: Option<Deployment>) -> MockDeploymentRepository {
        let mut mock = MockDeploymentRepository::new();
        mock.expect_get_by_id().returning(move |_| {
            let found = found.clone();
            Box::pin(async move { Ok(found) })
        });
        mock
    }

    fn service(
        found: Option<Deployment>,
        allowed: bool,
        audit: SpyAudit,
    ) -> LogServiceImpl<MockDeploymentRepository, SpyAudit, StubUsers, StubPolicy, NoRelay> {
        LogServiceImpl::new(
            repository(found),
            audit,
            StubUsers,
            StubPolicy(allowed),
            NoRelay,
        )
    }

    fn command() -> ReadLogsCommand {
        ReadLogsCommand {
            organisation_id: OrganisationId(ORGANISATION),
            deployment_id: DeploymentId(DEPLOYMENT),
            window: LogWindow::minutes(15).expect("inside the cap"),
        }
    }

    #[tokio::test]
    async fn an_allowed_read_opens_a_session_for_the_deployment() {
        let audit = SpyAudit::default();
        let accepted = service(Some(deployment(ORGANISATION)), true, audit.clone())
            .accept_read(caller(), command())
            .await
            .expect("allowed");

        assert_eq!(accepted.deployment.id.0, DEPLOYMENT);
        assert_eq!(accepted.session.window.as_minutes(), 15);
    }

    /// Reading someone's logs must leave a trace, and it must be written
    /// before a line moves: a trail appended afterwards is missing exactly
    /// the reads that failed halfway.
    #[tokio::test]
    async fn every_read_leaves_an_entry_naming_who_looked() {
        let audit = SpyAudit::default();
        service(Some(deployment(ORGANISATION)), true, audit.clone())
            .accept_read(caller(), command())
            .await
            .expect("allowed");

        let written = audit.0.lock().expect("not poisoned");
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].action.0, "deployment.logs.read");
        assert!(matches!(written[0].actor, AuditActor::User { user_id } if user_id == USER));
        assert_eq!(written[0].target.id, DEPLOYMENT);
    }

    /// The refusal comes before the deployment is read, so a caller with no
    /// right to these logs does not learn whether the instance exists.
    #[tokio::test]
    async fn a_caller_without_the_permission_is_refused_and_recorded_nowhere() {
        let audit = SpyAudit::default();
        let error = service(Some(deployment(ORGANISATION)), false, audit.clone())
            .accept_read(caller(), command())
            .await
            .expect_err("refused");

        assert!(matches!(error, CoreError::PermissionDenied { .. }));
        assert!(audit.0.lock().expect("not poisoned").is_empty());
    }

    #[tokio::test]
    async fn another_organisations_deployment_is_not_found() {
        let audit = SpyAudit::default();
        let error = service(Some(deployment(Uuid::from_u128(99))), true, audit.clone())
            .accept_read(caller(), command())
            .await
            .expect_err("not theirs");

        assert!(matches!(error, CoreError::DeploymentNotFound { .. }));
        assert!(audit.0.lock().expect("not poisoned").is_empty());
    }

    /// What the data plane is told carries the session and the window, and
    /// nothing about a line: there are no lines yet, and there never will be
    /// any here.
    #[tokio::test]
    async fn the_request_handed_to_the_data_plane_names_the_session_and_the_window() {
        let accepted = service(Some(deployment(ORGANISATION)), true, SpyAudit::default())
            .accept_read(caller(), command())
            .await
            .expect("allowed");

        let payload = log_request_payload(&accepted);

        assert_eq!(payload["session_id"], accepted.session.id.0.to_string());
        assert_eq!(payload["since_minutes"], 15);
        assert_eq!(payload["namespace"], "tenant-a");
    }
}
