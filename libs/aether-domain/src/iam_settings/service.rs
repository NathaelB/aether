use aether_auth::Identity;
use serde_json::json;

use crate::{
    CoreError,
    action::{
        Action, ActionPayload, ActionSource, ActionTarget, ActionType, ActionVersion, TargetKind,
        commands::{FetchActionsCommand, RecordActionCommand},
        ports::{ActionRepository, ActionService},
        service::ActionServiceImpl,
    },
    deployments::{Deployment, DeploymentId, ports::DeploymentRepository},
    iam_settings::{
        IAM_SETTINGS_ACTION_TYPE, IamSettings,
        ports::{IamSettingsPolicy, IamSettingsRequest, IamSettingsService, IamSettingsState},
    },
    organisation::{
        OrganisationId,
        features::{IamFeature, cheapest_plan_opening},
        ports::OrganisationRepository,
        value_objects::Plan,
    },
};

const ACTIONS_PER_PAGE: usize = 200;

pub struct IamSettingsServiceImpl<D, A, O, P>
where
    A: ActionRepository,
{
    deployment_repository: D,
    actions: ActionServiceImpl<A>,
    organisation_repository: O,
    policy: P,
}

impl<D, A, O, P> IamSettingsServiceImpl<D, A, O, P>
where
    A: ActionRepository,
{
    pub fn new(
        deployment_repository: D,
        action_repository: A,
        organisation_repository: O,
        policy: P,
    ) -> Self {
        Self {
            deployment_repository,
            actions: ActionServiceImpl::new(action_repository),
            organisation_repository,
            policy,
        }
    }
}

fn payload(deployment: &Deployment) -> serde_json::Value {
    json!({
        "deployment_id": deployment.id.0,
        "namespace": deployment.namespace,
        "branding": deployment.iam_settings.branding,
    })
}

fn require_open(
    plan: Plan,
    feature: IamFeature,
    opens: impl Fn(Plan, IamFeature) -> bool,
) -> Result<(), CoreError> {
    if opens(plan, feature) {
        return Ok(());
    }

    let reason = match cheapest_plan_opening(feature) {
        Some(opened_by) => {
            format!("the {plan} plan does not open {feature}: it is available from {opened_by}")
        }
        None => format!("the {plan} plan does not open {feature}"),
    };

    Err(CoreError::PermissionDenied { reason })
}

fn request_of(action: &Action) -> IamSettingsRequest {
    IamSettingsRequest {
        action_id: action.id,
        status: action.status.clone(),
        created_at: action.metadata.created_at,
    }
}

impl<D, A, O, P> IamSettingsServiceImpl<D, A, O, P>
where
    D: DeploymentRepository,
    A: ActionRepository,
    O: OrganisationRepository,
    P: IamSettingsPolicy,
{
    async fn deployment(
        &self,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<Deployment, CoreError> {
        self.deployment_repository
            .get_by_id(deployment_id)
            .await?
            .filter(|deployment| deployment.organisation_id == organisation_id)
            .filter(|deployment| deployment.deleted_at.is_none())
            .ok_or(CoreError::DeploymentNotFound {
                id: deployment_id.0,
            })
    }

    async fn latest_request(
        &self,
        deployment_id: DeploymentId,
    ) -> Result<Option<IamSettingsRequest>, CoreError> {
        let mut latest = None;
        let mut cursor = None;

        loop {
            let mut command = FetchActionsCommand::new(deployment_id, ACTIONS_PER_PAGE);
            command.cursor = cursor;
            let batch = self.actions.fetch_actions(command).await?;

            if let Some(action) = batch
                .actions
                .iter()
                .rev()
                .find(|action| action.action_type.0 == IAM_SETTINGS_ACTION_TYPE)
            {
                latest = Some(request_of(action));
            }

            match batch.next_cursor {
                Some(next) => cursor = Some(next),
                None => return Ok(latest),
            }
        }
    }

    async fn set_when(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        settings: IamSettings,
        opens: impl Fn(Plan, IamFeature) -> bool,
    ) -> Result<IamSettingsState, CoreError> {
        self.policy
            .can_change_iam_settings(identity, organisation_id)
            .await?;

        let mut deployment = self.deployment(organisation_id, deployment_id).await?;

        let organisation = self
            .organisation_repository
            .find_by_id(&organisation_id)
            .await?
            .ok_or(CoreError::OrganisationNotFound {
                id: organisation_id.0,
            })?;
        if settings.branding.is_some() {
            require_open(organisation.plan, IamFeature::Branding, opens)?;
        }

        deployment.iam_settings = settings;
        deployment.updated_at = chrono::Utc::now();
        self.deployment_repository
            .update(deployment.clone())
            .await?;

        let action = self
            .actions
            .record_action(RecordActionCommand::new(
                deployment.id,
                deployment.dataplane_id,
                ActionType(IAM_SETTINGS_ACTION_TYPE.to_string()),
                ActionTarget {
                    kind: TargetKind::Deployment,
                    id: deployment.id.0,
                },
                ActionPayload {
                    data: payload(&deployment),
                },
                ActionVersion(1),
                ActionSource::System,
            ))
            .await?;

        Ok(IamSettingsState {
            branding: deployment.iam_settings.branding,
            request: Some(request_of(&action)),
        })
    }
}

impl<D, A, O, P> IamSettingsService for IamSettingsServiceImpl<D, A, O, P>
where
    D: DeploymentRepository,
    A: ActionRepository,
    O: OrganisationRepository,
    P: IamSettingsPolicy,
{
    async fn iam_settings(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
    ) -> Result<IamSettingsState, CoreError> {
        self.policy
            .can_view_iam_settings(identity, organisation_id)
            .await?;

        let deployment = self.deployment(organisation_id, deployment_id).await?;

        Ok(IamSettingsState {
            branding: deployment.iam_settings.branding,
            request: self.latest_request(deployment_id).await?,
        })
    }

    async fn set_iam_settings(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
        deployment_id: DeploymentId,
        settings: IamSettings,
    ) -> Result<IamSettingsState, CoreError> {
        self.set_when(
            identity,
            organisation_id,
            deployment_id,
            settings,
            |plan, feature| plan.opens(feature),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use chrono::{DateTime, Duration, Utc};
    use uuid::Uuid;

    use super::*;
    use crate::{
        action::{
            ActionBatch, ActionCursor, ActionId, ActionMetadata, ActionStatus,
            ports::MockActionRepository,
        },
        dataplane::value_objects::{DataPlaneId, DeploymentResources},
        deployments::{
            DeploymentKind, DeploymentName, DeploymentStatus, ports::MockDeploymentRepository,
        },
        iam_settings::branding::{Branding, BrandingInput},
        organisation::{
            Organisation,
            ports::MockOrganisationRepository,
            value_objects::{OrganisationName, OrganisationSlug},
        },
        user::UserId,
        version::Version,
    };

    const ORGANISATION: Uuid = Uuid::from_u128(1);
    const DEPLOYMENT: Uuid = Uuid::from_u128(2);

    #[derive(Clone)]
    struct StubPolicy {
        allowed: bool,
        asked: Arc<Mutex<Vec<&'static str>>>,
    }

    impl StubPolicy {
        fn allowing() -> Self {
            Self {
                allowed: true,
                asked: Arc::default(),
            }
        }

        fn refusing() -> Self {
            Self {
                allowed: false,
                asked: Arc::default(),
            }
        }

        fn answer(&self, right: &'static str) -> Result<(), CoreError> {
            self.asked.lock().expect("not poisoned").push(right);
            if self.allowed {
                Ok(())
            } else {
                Err(CoreError::PermissionDenied {
                    reason: "insufficient permissions".to_string(),
                })
            }
        }
    }

    impl IamSettingsPolicy for StubPolicy {
        async fn can_view_iam_settings(
            &self,
            _identity: Identity,
            _organisation_id: OrganisationId,
        ) -> Result<(), CoreError> {
            self.answer("view")
        }

        async fn can_change_iam_settings(
            &self,
            _identity: Identity,
            _organisation_id: OrganisationId,
        ) -> Result<(), CoreError> {
            self.answer("change")
        }
    }

    fn caller() -> Identity {
        Identity::User(aether_auth::User {
            id: "user".to_string(),
            username: "user".to_string(),
            email: None,
            name: None,
            roles: vec![],
        })
    }

    fn deployment() -> Deployment {
        let at = Utc::now();
        Deployment {
            id: DeploymentId(DEPLOYMENT),
            organisation_id: OrganisationId(ORGANISATION),
            dataplane_id: DataPlaneId(Uuid::from_u128(3)),
            name: DeploymentName("auth".to_string()),
            kind: DeploymentKind::Ferriskey,
            version: Version::new(26, 0, 1),
            status: DeploymentStatus::Successful,
            namespace: "production-auth".to_string(),
            environment: crate::deployments::environment::Environment::Development,
            offer: None,
            restored_from: None,
            resources: DeploymentResources::DEFAULT,
            created_by: UserId(Uuid::from_u128(4)),
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
            iam_settings: Default::default(),
        }
    }

    fn branding(primary: &str, radius: i64) -> Branding {
        let mut input = BrandingInput::default();
        input.colors.primary = Some(primary.to_string());
        input.radius = Some(radius);
        Branding::try_from(input).expect("valid")
    }

    fn settings(branding: Option<Branding>) -> IamSettings {
        IamSettings { branding }
    }

    #[derive(Clone, Default)]
    struct Writes {
        deployments: Arc<Mutex<Vec<Deployment>>>,
        actions: Arc<Mutex<Vec<Action>>>,
    }

    fn deployments(
        found: Option<Deployment>,
        writes: &Writes,
        fail_update: bool,
    ) -> MockDeploymentRepository {
        let mut mock = MockDeploymentRepository::new();
        mock.expect_get_by_id().returning(move |_| {
            let found = found.clone();
            Box::pin(async move { Ok(found) })
        });
        let held = Arc::clone(&writes.deployments);
        mock.expect_update().returning(move |deployment| {
            held.lock().expect("not poisoned").push(deployment);
            Box::pin(async move {
                if fail_update {
                    Err(CoreError::DatabaseError {
                        message: "down".to_string(),
                    })
                } else {
                    Ok(())
                }
            })
        });
        mock
    }

    fn actions(writes: &Writes) -> MockActionRepository {
        let mut mock = MockActionRepository::new();
        let held = Arc::clone(&writes.actions);
        mock.expect_append().returning(move |action| {
            held.lock().expect("not poisoned").push(action);
            Box::pin(async { Ok(()) })
        });
        mock
    }

    fn organisations(plan: Plan) -> MockOrganisationRepository {
        let mut mock = MockOrganisationRepository::new();
        mock.expect_find_by_id().returning(move |id| {
            let mut organisation = Organisation::new(
                OrganisationName::new("FerrisLabs").expect("a name"),
                OrganisationSlug::new("ferrislabs").expect("a slug"),
                UserId(Uuid::from_u128(7)),
                plan,
            );
            organisation.id = *id;
            Box::pin(async move { Ok(Some(organisation)) })
        });
        mock
    }

    fn service(
        found: Option<Deployment>,
        writes: &Writes,
        policy: StubPolicy,
    ) -> IamSettingsServiceImpl<
        MockDeploymentRepository,
        MockActionRepository,
        MockOrganisationRepository,
        StubPolicy,
    > {
        IamSettingsServiceImpl::new(
            deployments(found, writes, false),
            actions(writes),
            organisations(Plan::Free),
            policy,
        )
    }

    async fn set(
        service: &IamSettingsServiceImpl<
            MockDeploymentRepository,
            MockActionRepository,
            MockOrganisationRepository,
            StubPolicy,
        >,
        value: IamSettings,
    ) -> Result<IamSettingsState, CoreError> {
        service
            .set_iam_settings(
                caller(),
                OrganisationId(ORGANISATION),
                DeploymentId(DEPLOYMENT),
                value,
            )
            .await
    }

    fn nothing_was_written(writes: &Writes) {
        assert!(writes.deployments.lock().expect("not poisoned").is_empty());
        assert!(writes.actions.lock().expect("not poisoned").is_empty());
    }

    #[tokio::test]
    async fn setting_stores_the_state_and_creates_one_action_carrying_all_of_it() {
        let writes = Writes::default();
        let subject = service(Some(deployment()), &writes, StubPolicy::allowing());
        let wanted = branding("#112233", 6);

        let state = set(&subject, settings(Some(wanted.clone())))
            .await
            .expect("accepted");

        let stored = writes.deployments.lock().expect("not poisoned");
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].iam_settings.branding, Some(wanted.clone()));

        let recorded = writes.actions.lock().expect("not poisoned");
        assert_eq!(recorded.len(), 1);
        let action = &recorded[0];
        assert_eq!(action.action_type.0, "deployment.iam_settings");
        assert_eq!(action.version, ActionVersion(1));
        assert_eq!(action.deployment_id, Some(DeploymentId(DEPLOYMENT)));
        assert_eq!(action.dataplane_id, DataPlaneId(Uuid::from_u128(3)));
        assert_eq!(action.target.kind, TargetKind::Deployment);
        assert_eq!(action.target.id, DEPLOYMENT);
        assert_eq!(action.metadata.source, ActionSource::System);
        assert_eq!(action.status, ActionStatus::Pending);
        assert_eq!(
            action.payload.data,
            json!({
                "deployment_id": DEPLOYMENT,
                "namespace": "production-auth",
                "branding": { "colors": { "primary": "#112233" }, "radius": 6 },
            })
        );

        assert_eq!(state.branding, Some(wanted));
        let request = state.request.expect("the request just made");
        assert_eq!(request.action_id, action.id);
        assert_eq!(request.status, ActionStatus::Pending);
        assert_eq!(request.created_at, action.metadata.created_at);
    }

    #[tokio::test]
    async fn setting_null_stores_nothing_wanted_and_sends_a_null_branding() {
        let writes = Writes::default();
        let mut existing = deployment();
        existing.iam_settings = settings(Some(branding("#112233", 6)));
        let subject = service(Some(existing), &writes, StubPolicy::allowing());

        let state = set(&subject, settings(None)).await.expect("accepted");

        assert_eq!(
            writes.deployments.lock().expect("not poisoned")[0].iam_settings,
            IamSettings::default()
        );
        let recorded = writes.actions.lock().expect("not poisoned");
        assert_eq!(recorded.len(), 1);
        assert_eq!(
            recorded[0].payload.data,
            json!({
                "deployment_id": DEPLOYMENT,
                "namespace": "production-auth",
                "branding": null,
            })
        );
        assert_eq!(state.branding, None);
    }

    #[tokio::test]
    async fn setting_twice_sends_the_whole_state_each_time() {
        let writes = Writes::default();
        let subject = service(Some(deployment()), &writes, StubPolicy::allowing());

        set(&subject, settings(Some(branding("#112233", 6))))
            .await
            .expect("first");
        set(&subject, settings(Some(branding("#112233", 6))))
            .await
            .expect("second");

        let recorded = writes.actions.lock().expect("not poisoned");
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0].payload, recorded[1].payload);
        assert_ne!(recorded[0].id, recorded[1].id);
    }

    #[tokio::test]
    async fn a_caller_without_the_right_changes_and_creates_nothing() {
        let writes = Writes::default();
        let policy = StubPolicy::refusing();
        let subject = service(Some(deployment()), &writes, policy.clone());

        let refused = set(&subject, settings(None)).await.expect_err("refused");

        assert!(matches!(refused, CoreError::PermissionDenied { .. }));
        assert_eq!(*policy.asked.lock().expect("not poisoned"), vec!["change"]);
        nothing_was_written(&writes);
    }

    #[tokio::test]
    async fn another_organisations_deployment_is_not_found_and_nothing_is_created() {
        let writes = Writes::default();
        let mut foreign = deployment();
        foreign.organisation_id = OrganisationId(Uuid::from_u128(99));
        let subject = service(Some(foreign), &writes, StubPolicy::allowing());

        let refused = set(&subject, settings(None)).await.expect_err("not found");

        assert!(matches!(refused, CoreError::DeploymentNotFound { .. }));
        nothing_was_written(&writes);
    }

    #[tokio::test]
    async fn a_missing_or_deleted_deployment_is_not_found() {
        let writes = Writes::default();
        let subject = service(None, &writes, StubPolicy::allowing());
        assert!(matches!(
            set(&subject, settings(None)).await,
            Err(CoreError::DeploymentNotFound { .. })
        ));

        let mut deleted = deployment();
        deleted.deleted_at = Some(Utc::now());
        let subject = service(Some(deleted), &writes, StubPolicy::allowing());
        assert!(matches!(
            set(&subject, settings(None)).await,
            Err(CoreError::DeploymentNotFound { .. })
        ));
        nothing_was_written(&writes);
    }

    #[tokio::test]
    async fn a_plan_that_does_not_open_branding_is_refused_before_anything_is_written() {
        let writes = Writes::default();
        let subject = service(Some(deployment()), &writes, StubPolicy::allowing());

        let refused = subject
            .set_when(
                caller(),
                OrganisationId(ORGANISATION),
                DeploymentId(DEPLOYMENT),
                settings(Some(branding("#112233", 6))),
                |_, feature| feature != IamFeature::Branding,
            )
            .await
            .expect_err("closed");

        let CoreError::PermissionDenied { reason } = refused else {
            panic!("expected a permission refusal");
        };
        assert!(reason.contains("free"), "{reason}");
        assert!(reason.contains("branding"), "{reason}");
        nothing_was_written(&writes);
    }

    #[tokio::test]
    async fn a_plan_that_closes_branding_still_lets_a_customer_clear_it() {
        let writes = Writes::default();
        let subject = service(Some(deployment()), &writes, StubPolicy::allowing());

        subject
            .set_when(
                caller(),
                OrganisationId(ORGANISATION),
                DeploymentId(DEPLOYMENT),
                settings(None),
                |_, feature| feature != IamFeature::Branding,
            )
            .await
            .expect("clearing is always allowed");

        assert_eq!(writes.actions.lock().expect("not poisoned").len(), 1);
    }

    #[tokio::test]
    async fn another_feature_being_closed_does_not_refuse_branding() {
        let writes = Writes::default();
        let subject = service(Some(deployment()), &writes, StubPolicy::allowing());

        subject
            .set_when(
                caller(),
                OrganisationId(ORGANISATION),
                DeploymentId(DEPLOYMENT),
                settings(None),
                |_, feature| feature == IamFeature::Branding,
            )
            .await
            .expect("open");

        assert_eq!(writes.actions.lock().expect("not poisoned").len(), 1);
    }

    #[test]
    fn the_gate_names_the_plan_and_the_tier_that_opens_the_feature() {
        let refused =
            require_open(Plan::Free, IamFeature::Branding, |_, _| false).expect_err("closed");

        let CoreError::PermissionDenied { reason } = refused else {
            panic!("expected a permission refusal");
        };
        assert!(reason.contains("available from"), "{reason}");
        assert!(require_open(Plan::Free, IamFeature::Branding, |plan, f| plan.opens(f)).is_ok());
    }

    #[tokio::test]
    async fn an_organisation_that_does_not_exist_creates_nothing() {
        let writes = Writes::default();
        let mut missing = MockOrganisationRepository::new();
        missing
            .expect_find_by_id()
            .returning(|_| Box::pin(async { Ok(None) }));
        let subject = IamSettingsServiceImpl::new(
            deployments(Some(deployment()), &writes, false),
            actions(&writes),
            missing,
            StubPolicy::allowing(),
        );

        let refused = set(&subject, settings(None)).await.expect_err("missing");

        assert!(matches!(refused, CoreError::OrganisationNotFound { .. }));
        nothing_was_written(&writes);
    }

    #[tokio::test]
    async fn a_failed_write_creates_no_action() {
        let writes = Writes::default();
        let subject = IamSettingsServiceImpl::new(
            deployments(Some(deployment()), &writes, true),
            actions(&writes),
            organisations(Plan::Free),
            StubPolicy::allowing(),
        );

        let failed = set(&subject, settings(None)).await.expect_err("down");

        assert!(matches!(failed, CoreError::DatabaseError { .. }));
        assert!(writes.actions.lock().expect("not poisoned").is_empty());
    }

    fn recorded_action(kind: &str, status: ActionStatus, created_at: DateTime<Utc>) -> Action {
        Action {
            id: ActionId(Uuid::new_v4()),
            deployment_id: Some(DeploymentId(DEPLOYMENT)),
            dataplane_id: DataPlaneId(Uuid::from_u128(3)),
            action_type: ActionType(kind.to_string()),
            target: ActionTarget {
                kind: TargetKind::Deployment,
                id: DEPLOYMENT,
            },
            payload: ActionPayload {
                data: serde_json::Value::Null,
            },
            version: ActionVersion(1),
            status,
            metadata: ActionMetadata {
                source: ActionSource::System,
                created_at,
                constraints: Default::default(),
            },
            leased_until: None,
        }
    }

    fn history(pages: Vec<Vec<Action>>) -> MockActionRepository {
        let pages = Arc::new(pages);
        let mut mock = MockActionRepository::new();
        mock.expect_list().returning(move |_, cursor, _| {
            let index = cursor
                .map(|cursor| cursor.0.parse::<usize>().expect("a page"))
                .unwrap_or(0);
            let actions = pages[index].clone();
            let next_cursor =
                (index + 1 < pages.len()).then(|| ActionCursor::new(format!("{}", index + 1)));
            Box::pin(async move {
                Ok(ActionBatch {
                    actions,
                    next_cursor,
                })
            })
        });
        mock
    }

    async fn read(
        found: Option<Deployment>,
        history: MockActionRepository,
        policy: StubPolicy,
    ) -> Result<IamSettingsState, CoreError> {
        IamSettingsServiceImpl::new(
            deployments(found, &Writes::default(), false),
            history,
            organisations(Plan::Free),
            policy,
        )
        .iam_settings(
            caller(),
            OrganisationId(ORGANISATION),
            DeploymentId(DEPLOYMENT),
        )
        .await
    }

    #[tokio::test]
    async fn reading_answers_the_desired_state_and_the_latest_request_across_pages() {
        let at = Utc::now();
        let mut existing = deployment();
        existing.iam_settings = settings(Some(branding("#abcdef", 3)));
        let latest = recorded_action(
            IAM_SETTINGS_ACTION_TYPE,
            ActionStatus::Published {
                at: at + Duration::seconds(5),
            },
            at + Duration::seconds(2),
        );
        let pages = vec![
            vec![
                recorded_action(IAM_SETTINGS_ACTION_TYPE, ActionStatus::Pending, at),
                recorded_action("deployment.network_access", ActionStatus::Pending, at),
            ],
            vec![
                latest.clone(),
                recorded_action(
                    "deployment.network_access",
                    ActionStatus::Pending,
                    at + Duration::seconds(3),
                ),
            ],
        ];

        let state = read(Some(existing), history(pages), StubPolicy::allowing())
            .await
            .expect("read");

        assert_eq!(state.branding, Some(branding("#abcdef", 3)));
        let request = state.request.expect("a request");
        assert_eq!(request.action_id, latest.id);
        assert_eq!(request.status, latest.status);
        assert_eq!(request.created_at, latest.metadata.created_at);
    }

    #[tokio::test]
    async fn reading_with_no_request_made_says_so() {
        let pages = vec![vec![recorded_action(
            "deployment.network_access",
            ActionStatus::Pending,
            Utc::now(),
        )]];

        let state = read(Some(deployment()), history(pages), StubPolicy::allowing())
            .await
            .expect("read");

        assert_eq!(state.branding, None);
        assert_eq!(state.request, None);
    }

    #[tokio::test]
    async fn a_failed_request_is_shown_as_failed() {
        let failed = ActionStatus::Failed {
            reason: crate::action::ActionFailureReason::PublishFailed,
            at: Utc::now(),
        };
        let pages = vec![vec![recorded_action(
            IAM_SETTINGS_ACTION_TYPE,
            failed.clone(),
            Utc::now(),
        )]];

        let state = read(Some(deployment()), history(pages), StubPolicy::allowing())
            .await
            .expect("read");

        assert_eq!(state.request.expect("a request").status, failed);
    }

    #[tokio::test]
    async fn reading_needs_the_view_right_and_scopes_to_the_organisation() {
        let policy = StubPolicy::refusing();
        let refused = read(
            Some(deployment()),
            MockActionRepository::new(),
            policy.clone(),
        )
        .await
        .expect_err("refused");
        assert!(matches!(refused, CoreError::PermissionDenied { .. }));
        assert_eq!(*policy.asked.lock().expect("not poisoned"), vec!["view"]);

        let mut foreign = deployment();
        foreign.organisation_id = OrganisationId(Uuid::from_u128(99));
        let absent = read(
            Some(foreign),
            MockActionRepository::new(),
            StubPolicy::allowing(),
        )
        .await
        .expect_err("not found");
        assert!(matches!(absent, CoreError::DeploymentNotFound { .. }));
    }
}
