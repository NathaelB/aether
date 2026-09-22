use chrono::Utc;
use uuid::Uuid;

use aether_auth::Identity;

use crate::{
    CoreError,
    audit::{
        AuditAction, AuditEntry, AuditEntryId, AuditTarget, AuditTargetKind,
        ports::AuditRepository, service::audit_actor,
    },
    deployments::ports::DeploymentRepository,
    logs::ports::LogPolicy,
    traces::{
        TraceDetail, TraceSearchFilter, TraceSearchResult,
        commands::{ReadTraceCommand, SearchTracesCommand},
        ports::TraceSearchIndex,
    },
    user::ports::UserRepository,
};

/// Reads an organisation's trace index. Reuses [`LogPolicy`] rather than
/// introducing a trace-specific permission: whether an identity may read a
/// deployment's OpenTelemetry data is the same question `can_read_logs`
/// already answers for its logs, and a second permission that always
/// tracks the first would be a permission in name only.
pub struct TraceSearchServiceImpl<D, A, U, P, S>
where
    D: DeploymentRepository,
    A: AuditRepository,
    U: UserRepository,
    P: LogPolicy,
    S: TraceSearchIndex,
{
    deployment_repository: D,
    audit_repository: A,
    user_repository: U,
    policy: P,
    search_index: S,
}

impl<D, A, U, P, S> TraceSearchServiceImpl<D, A, U, P, S>
where
    D: DeploymentRepository,
    A: AuditRepository,
    U: UserRepository,
    P: LogPolicy,
    S: TraceSearchIndex,
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

    /// Refuses before the index is ever asked anything -- the same guarantee
    /// `LogSearchServiceImpl::search` gives: a caller without the right to
    /// this organisation's observability data, or a deployment filter naming
    /// one that is not theirs, never reaches [`TraceSearchIndex::search`].
    pub async fn search(
        &self,
        identity: Identity,
        command: SearchTracesCommand,
    ) -> Result<TraceSearchResult, CoreError> {
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
                AuditAction("organisation.traces.searched".to_string()),
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
                TraceSearchFilter {
                    deployment_id: command.deployment_id,
                    window: command.window,
                    service_name: command.service_name,
                    status_code: command.status_code,
                    text: command.text,
                },
            )
            .await
    }

    /// Every span of one trace, for the waterfall view. Not scoped to a
    /// deployment: a reader already holds a `trace_id` because a search hit
    /// (already authorised, already audited) named it, so this only needs
    /// the organisation-level permission and its own audit entry.
    pub async fn trace(
        &self,
        identity: Identity,
        command: ReadTraceCommand,
    ) -> Result<TraceDetail, CoreError> {
        self.policy
            .can_read_logs(identity.clone(), command.organisation_id)
            .await?;

        let actor = audit_actor(&identity, &self.user_repository).await?;
        self.audit_repository
            .append(AuditEntry::record(
                AuditEntryId(Uuid::new_v4()),
                command.organisation_id,
                actor,
                AuditAction("organisation.traces.read".to_string()),
                AuditTarget {
                    kind: AuditTargetKind::Organisation,
                    id: command.organisation_id.0,
                },
                None,
                Utc::now(),
            ))
            .await?;

        self.search_index
            .trace(command.organisation_id, command.trace_id)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        audit::{AuditActor, AuditEntry},
        dataplane::value_objects::DataPlaneId,
        deployments::{
            Deployment, DeploymentId, DeploymentKind, DeploymentName, DeploymentStatus,
            ports::MockDeploymentRepository,
        },
        organisation::OrganisationId,
        traces::ports::MockTraceSearchIndex,
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
            _organisation_id: OrganisationId,
            _cursor: Option<crate::audit::AuditCursor>,
            _limit: usize,
        ) -> Result<crate::audit::AuditBatch, CoreError> {
            unreachable!("a trace search never lists the trail")
        }
    }

    #[derive(Clone)]
    struct StubUsers;

    impl UserRepository for StubUsers {
        async fn upsert_by_email(&self, _user: &User) -> Result<User, CoreError> {
            unreachable!("a trace search never writes a user")
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
                    reason: "not allowed to read traces".to_string(),
                })
            }
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
            resources: crate::dataplane::value_objects::DeploymentResources::DEFAULT,
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
        search_index: MockTraceSearchIndex,
    ) -> TraceSearchServiceImpl<
        MockDeploymentRepository,
        SpyAudit,
        StubUsers,
        StubPolicy,
        MockTraceSearchIndex,
    > {
        TraceSearchServiceImpl::new(
            repository(found),
            audit,
            StubUsers,
            StubPolicy(allowed),
            search_index,
        )
    }

    fn instant(text: &str) -> chrono::DateTime<Utc> {
        chrono::DateTime::parse_from_rfc3339(text)
            .expect("a valid instant")
            .with_timezone(&Utc)
    }

    fn window() -> crate::logs::LogSearchWindow {
        crate::logs::LogSearchWindow::new(
            instant("2026-09-01T00:00:00Z"),
            instant("2026-09-02T00:00:00Z"),
        )
        .expect("an ordinary window")
    }

    fn search_command(deployment_id: Option<DeploymentId>) -> SearchTracesCommand {
        SearchTracesCommand {
            organisation_id: OrganisationId(ORGANISATION),
            deployment_id,
            window: window(),
            service_name: None,
            status_code: None,
            text: None,
        }
    }

    fn empty_result() -> TraceSearchResult {
        TraceSearchResult {
            hits: vec![],
            total_hits: 0,
            facets: crate::traces::TraceFacets {
                service_name: vec![],
                status_code: vec![],
            },
            buckets: vec![],
        }
    }

    /// The same isolation guarantee `LogSearchServiceImpl`'s tests
    /// demonstrate: a mock with no expectation set panics the moment
    /// anything calls it, so a permission refusal that reached the index
    /// would fail this test on its own.
    #[tokio::test]
    async fn a_caller_without_the_permission_is_refused_and_the_index_is_never_asked() {
        let error = service(
            Some(deployment(ORGANISATION)),
            false,
            SpyAudit::default(),
            MockTraceSearchIndex::new(),
        )
        .search(caller(), search_command(None))
        .await
        .expect_err("refused");

        assert!(matches!(error, CoreError::PermissionDenied { .. }));
    }

    #[tokio::test]
    async fn a_deployment_from_another_organisation_is_refused_and_the_index_is_never_asked() {
        let error = service(
            Some(deployment(Uuid::from_u128(99))),
            true,
            SpyAudit::default(),
            MockTraceSearchIndex::new(),
        )
        .search(caller(), search_command(Some(DeploymentId(DEPLOYMENT))))
        .await
        .expect_err("not theirs");

        assert!(matches!(error, CoreError::DeploymentNotFound { .. }));
    }

    #[tokio::test]
    async fn an_allowed_search_reaches_the_index_with_the_translated_filter() {
        let mut index = MockTraceSearchIndex::new();
        index
            .expect_search()
            .times(1)
            .withf(|organisation_id, filter| {
                *organisation_id == OrganisationId(ORGANISATION) && filter.deployment_id.is_none()
            })
            .returning(|_, _| Box::pin(async { Ok(empty_result()) }));

        service(
            Some(deployment(ORGANISATION)),
            true,
            SpyAudit::default(),
            index,
        )
        .search(caller(), search_command(None))
        .await
        .expect("allowed");
    }

    #[tokio::test]
    async fn every_search_leaves_an_audit_entry() {
        let audit = SpyAudit::default();
        let mut index = MockTraceSearchIndex::new();
        index
            .expect_search()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(empty_result()) }));

        service(Some(deployment(ORGANISATION)), true, audit.clone(), index)
            .search(caller(), search_command(None))
            .await
            .expect("allowed");

        let written = audit.0.lock().expect("not poisoned");
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].action.0, "organisation.traces.searched");
        assert!(matches!(written[0].actor, AuditActor::User { user_id } if user_id == USER));
    }

    #[tokio::test]
    async fn a_refused_search_is_recorded_nowhere() {
        let audit = SpyAudit::default();

        service(
            Some(deployment(ORGANISATION)),
            false,
            audit.clone(),
            MockTraceSearchIndex::new(),
        )
        .search(caller(), search_command(None))
        .await
        .expect_err("refused");

        assert!(audit.0.lock().expect("not poisoned").is_empty());
    }

    #[tokio::test]
    async fn a_trace_read_reaches_the_index_and_leaves_its_own_audit_action() {
        let audit = SpyAudit::default();
        let mut index = MockTraceSearchIndex::new();
        index
            .expect_trace()
            .times(1)
            .withf(|organisation_id, trace_id| {
                *organisation_id == OrganisationId(ORGANISATION) && trace_id == "abc123"
            })
            .returning(|_, trace_id| {
                Box::pin(async move {
                    Ok(TraceDetail {
                        trace_id,
                        spans: vec![],
                    })
                })
            });

        service(Some(deployment(ORGANISATION)), true, audit.clone(), index)
            .trace(
                caller(),
                ReadTraceCommand {
                    organisation_id: OrganisationId(ORGANISATION),
                    trace_id: "abc123".to_string(),
                },
            )
            .await
            .expect("allowed");

        let written = audit.0.lock().expect("not poisoned");
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].action.0, "organisation.traces.read");
    }

    #[tokio::test]
    async fn a_refused_trace_read_never_reaches_the_index() {
        let error = service(
            Some(deployment(ORGANISATION)),
            false,
            SpyAudit::default(),
            MockTraceSearchIndex::new(),
        )
        .trace(
            caller(),
            ReadTraceCommand {
                organisation_id: OrganisationId(ORGANISATION),
                trace_id: "abc123".to_string(),
            },
        )
        .await
        .expect_err("refused");

        assert!(matches!(error, CoreError::PermissionDenied { .. }));
    }
}
