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
        LogSearchFilter, LogSearchResult, LogSession,
        commands::{ReadLogsCommand, SearchLogsCommand},
        ports::{LogPolicy, LogRelay, LogSearchIndex},
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
            id: command.session_id,
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
        "organisation_id": accepted.deployment.organisation_id.0,
        "namespace": accepted.deployment.namespace.clone(),
        "kind": accepted.deployment.kind.to_string(),
        "session_id": accepted.session.id.0,
        "since_minutes": accepted.session.window.as_minutes(),
    })
}

pub struct LogSearchServiceImpl<D, A, U, P, S>
where
    D: DeploymentRepository,
    A: AuditRepository,
    U: UserRepository,
    P: LogPolicy,
    S: LogSearchIndex,
{
    deployment_repository: D,
    audit_repository: A,
    user_repository: U,
    policy: P,
    search_index: S,
}

impl<D, A, U, P, S> LogSearchServiceImpl<D, A, U, P, S>
where
    D: DeploymentRepository,
    A: AuditRepository,
    U: UserRepository,
    P: LogPolicy,
    S: LogSearchIndex,
{
    pub fn new(
        deployment_repository: D,
        audit_repository: A,
        user_repository: U,
        policy: P,
        search_index: S,
    ) -> Self {
        Self {
            deployment_repository,
            audit_repository,
            user_repository,
            policy,
            search_index,
        }
    }

    /// Refuses before the index is ever asked anything: a caller without the
    /// right to these logs, or a deployment filter naming one that is not
    /// this organisation's, never reaches [`LogSearchIndex::search`] at all --
    /// there is no path through this method that calls it before both checks
    /// have passed.
    pub async fn search(
        &self,
        identity: Identity,
        command: SearchLogsCommand,
    ) -> Result<LogSearchResult, CoreError> {
        self.policy
            .can_read_logs(identity.clone(), command.organisation_id)
            .await?;

        if let Some(deployment_id) = command.deployment_id {
            self.deployment_repository
                .get_by_id(deployment_id)
                .await?
                .filter(|deployment| deployment.organisation_id == command.organisation_id)
                .ok_or(CoreError::DeploymentNotFound {
                    id: deployment_id.0,
                })?;
        }

        let actor = audit_actor(&identity, &self.user_repository).await?;

        self.audit_repository
            .append(AuditEntry::record(
                AuditEntryId(Uuid::new_v4()),
                command.organisation_id,
                actor,
                AuditAction("organisation.logs.searched".to_string()),
                AuditTarget {
                    kind: AuditTargetKind::Organisation,
                    id: command.organisation_id.0,
                },
                None,
                Utc::now(),
            ))
            .await?;

        self.search_index
            .search(
                command.organisation_id,
                LogSearchFilter {
                    deployment_id: command.deployment_id,
                    window: command.window,
                    level_floor: command.level_floor,
                    text: command.text,
                },
            )
            .await
    }
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

        async fn find_by_email(&self, _email: &str) -> Result<Option<User>, CoreError> {
            unreachable!("this suite does not look anybody up by address")
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
        async fn next(&mut self) -> Option<crate::logs::Relayed> {
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
            _session_id: crate::logs::LogSessionId,
            _lines: Vec<LogLine>,
        ) -> Result<bool, CoreError> {
            Ok(true)
        }

        async fn end(
            &self,
            _session_id: crate::logs::LogSessionId,
            _end: crate::logs::SessionEnd,
        ) -> Result<(), CoreError> {
            Ok(())
        }

        async fn close(&self, _session_id: crate::logs::LogSessionId) -> Result<(), CoreError> {
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
            environment: crate::deployments::environment::Environment::Development,
            offer: None,
            restored_from: None,
            resources: DeploymentResources::DEFAULT,
            created_by: UserId(USER),
            created_at: at,
            updated_at: at,
            deployed_at: None,
            deleted_at: None,
            auto_upgrade: Default::default(),
            maintenance_window: None,
            network_access: crate::deployments::network::NetworkAccess::Open,
            last_verified_restore_at: None,
            last_restore_drill_seconds: None,
            log_shipping_enabled: false,
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
            session_id: LogSessionId(Uuid::from_u128(42)),
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
        assert_eq!(
            payload["organisation_id"],
            accepted.deployment.organisation_id.0.to_string()
        );
    }

    mod search {
        use super::*;
        use crate::logs::{
            LogFacets, LogLevel, LogSearchFilter, LogSearchHit, LogSearchResult, LogSearchWindow,
            ports::MockLogSearchIndex,
        };

        fn instant(text: &str) -> chrono::DateTime<Utc> {
            chrono::DateTime::parse_from_rfc3339(text)
                .expect("a valid instant")
                .with_timezone(&Utc)
        }

        fn window() -> LogSearchWindow {
            LogSearchWindow::new(
                instant("2026-09-01T00:00:00Z"),
                instant("2026-09-02T00:00:00Z"),
            )
            .expect("an ordinary window")
        }

        fn search_command(deployment_id: Option<DeploymentId>) -> SearchLogsCommand {
            SearchLogsCommand {
                organisation_id: OrganisationId(ORGANISATION),
                deployment_id,
                window: window(),
                level_floor: LogLevel::Warn,
                text: Some("pool exhausted".to_string()),
            }
        }

        fn search_service(
            found: Option<Deployment>,
            allowed: bool,
            audit: SpyAudit,
            search_index: MockLogSearchIndex,
        ) -> LogSearchServiceImpl<
            MockDeploymentRepository,
            SpyAudit,
            StubUsers,
            StubPolicy,
            MockLogSearchIndex,
        > {
            LogSearchServiceImpl::new(
                repository(found),
                audit,
                StubUsers,
                StubPolicy(allowed),
                search_index,
            )
        }

        fn empty_result() -> LogSearchResult {
            LogSearchResult {
                hits: vec![],
                total_hits: 0,
                facets: LogFacets {
                    level: vec![],
                    source: vec![],
                    deployment_id: vec![],
                },
            }
        }

        /// The isolation guarantee the issue asks for demonstrated directly:
        /// a mock with no expectation set panics the moment anything calls
        /// it, so a permission refusal that reached the index would fail this
        /// test on its own rather than needing an assertion to notice.
        #[tokio::test]
        async fn a_caller_without_the_permission_is_refused_and_the_index_is_never_asked() {
            let error = search_service(
                Some(deployment(ORGANISATION)),
                false,
                SpyAudit::default(),
                MockLogSearchIndex::new(),
            )
            .search(caller(), search_command(None))
            .await
            .expect_err("refused");

            assert!(matches!(error, CoreError::PermissionDenied { .. }));
        }

        /// Same guarantee for the other half of tenant isolation: a
        /// deployment filter naming somebody else's deployment is refused
        /// before the index -- which only knows this organisation's index
        /// exists in the first place -- is ever asked anything.
        #[tokio::test]
        async fn a_deployment_from_another_organisation_is_refused_and_the_index_is_never_asked() {
            let error = search_service(
                Some(deployment(Uuid::from_u128(99))),
                true,
                SpyAudit::default(),
                MockLogSearchIndex::new(),
            )
            .search(caller(), search_command(Some(DeploymentId(DEPLOYMENT))))
            .await
            .expect_err("not theirs");

            assert!(matches!(error, CoreError::DeploymentNotFound { .. }));
        }

        #[tokio::test]
        async fn an_allowed_search_reaches_the_index_with_the_translated_filter() {
            let mut index = MockLogSearchIndex::new();
            index
                .expect_search()
                .times(1)
                .withf(|organisation_id, filter| {
                    *organisation_id == OrganisationId(ORGANISATION)
                        && filter.deployment_id.is_none()
                        && filter.level_floor == LogLevel::Warn
                        && filter.text.as_deref() == Some("pool exhausted")
                })
                .returning(|_, _| Box::pin(async { Ok(empty_result()) }));

            search_service(
                Some(deployment(ORGANISATION)),
                true,
                SpyAudit::default(),
                index,
            )
            .search(caller(), search_command(None))
            .await
            .expect("allowed");
        }

        /// A deployment filter that does belong to the organisation is kept,
        /// not dropped, once ownership is confirmed.
        #[tokio::test]
        async fn a_deployment_filter_for_the_right_organisation_reaches_the_index() {
            let mut index = MockLogSearchIndex::new();
            index
                .expect_search()
                .times(1)
                .withf(|_, filter: &LogSearchFilter| {
                    filter.deployment_id == Some(DeploymentId(DEPLOYMENT))
                })
                .returning(|_, _| Box::pin(async { Ok(empty_result()) }));

            search_service(
                Some(deployment(ORGANISATION)),
                true,
                SpyAudit::default(),
                index,
            )
            .search(caller(), search_command(Some(DeploymentId(DEPLOYMENT))))
            .await
            .expect("allowed");
        }

        #[tokio::test]
        async fn a_successful_search_returns_whatever_the_index_answered() {
            let mut index = MockLogSearchIndex::new();
            index.expect_search().times(1).returning(|_, _| {
                Box::pin(async {
                    Ok(LogSearchResult {
                        hits: vec![LogSearchHit {
                            timestamp: instant("2026-09-01T08:00:00Z"),
                            deployment_id: DeploymentId(DEPLOYMENT),
                            source: "ferriskey-api".to_string(),
                            level: "warn".to_string(),
                            message: "the pool is exhausted".to_string(),
                        }],
                        total_hits: 1,
                        facets: LogFacets {
                            level: vec![],
                            source: vec![],
                            deployment_id: vec![],
                        },
                    })
                })
            });

            let result = search_service(
                Some(deployment(ORGANISATION)),
                true,
                SpyAudit::default(),
                index,
            )
            .search(caller(), search_command(None))
            .await
            .expect("allowed");

            assert_eq!(result.total_hits, 1);
            assert_eq!(result.hits[0].message, "the pool is exhausted");
        }

        /// Reading logs through search must leave the same kind of trace a
        /// live read does.
        #[tokio::test]
        async fn every_search_leaves_an_audit_entry() {
            let audit = SpyAudit::default();
            let mut index = MockLogSearchIndex::new();
            index
                .expect_search()
                .times(1)
                .returning(|_, _| Box::pin(async { Ok(empty_result()) }));

            search_service(Some(deployment(ORGANISATION)), true, audit.clone(), index)
                .search(caller(), search_command(None))
                .await
                .expect("allowed");

            let written = audit.0.lock().expect("not poisoned");
            assert_eq!(written.len(), 1);
            assert_eq!(written[0].action.0, "organisation.logs.searched");
            assert_eq!(written[0].target.id, ORGANISATION);
        }

        /// A refusal, from either check, writes nothing: the same rule the
        /// live tail follows.
        #[tokio::test]
        async fn a_refused_search_is_recorded_nowhere() {
            let audit = SpyAudit::default();

            search_service(
                Some(deployment(ORGANISATION)),
                false,
                audit.clone(),
                MockLogSearchIndex::new(),
            )
            .search(caller(), search_command(None))
            .await
            .expect_err("refused");

            assert!(audit.0.lock().expect("not poisoned").is_empty());
        }
    }
}
