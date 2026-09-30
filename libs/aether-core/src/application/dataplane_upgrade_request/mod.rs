use std::fmt;

use aether_auth::Identity;
use aether_domain::{
    CoreError,
    action::{
        Action, ActionSource, ActionStatus, ActionType, ActionVersion,
        commands::{FetchActionsCommand, RecordActionCommand},
        ports::{ActionRepository, ActionService},
        service::ActionServiceImpl,
    },
    dataplane::{entities::DataPlane, ports::DataPlaneRepository, value_objects::DataPlaneId},
    platform::{PlatformRight, ports::PlatformPolicy},
    version::{Version, VersionError},
};
use aether_macros::transactional;
use aether_postgres::registry::{Backend, RepoFor, domain};

use crate::{
    AetherService,
    application::dataplane_upgrade::{
        DataplaneUpgradePayload, InvalidDataplaneUpgrade, PAYLOAD_VERSION, UpgradeComponent,
        UpgradeStrategy,
    },
    policy::PlatformRightsPolicy,
};

const PAGE: usize = 100;

#[derive(Debug, Clone)]
pub struct DataplaneUpgradeRequest {
    pub target_version: String,
    pub dataplanes: Option<Vec<DataPlaneId>>,
    pub components: Vec<UpgradeComponent>,
    pub strategy: UpgradeStrategy,
    pub max_unavailable: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RequestedUpgrade {
    pub dataplane_id: DataPlaneId,
    pub action: Action,
    pub already_requested: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataplaneUpgrades {
    pub dataplane_id: DataPlaneId,
    pub actions: Vec<Action>,
}

#[derive(Debug)]
pub enum DataplaneUpgradeError {
    Invalid(InvalidDataplaneUpgrade),
    BadVersion(VersionError),
    UnknownDataplane(DataPlaneId),
    NoMinimumConfigured {
        dataplane: DataPlaneId,
    },
    HeraldTooOld {
        dataplane: DataPlaneId,
        reported: Option<Version>,
        minimum: Version,
    },
    AlreadyUpgrading {
        dataplane: DataPlaneId,
        running: Option<String>,
    },
    Core(CoreError),
}

impl fmt::Display for DataplaneUpgradeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(error) => error.fmt(f),
            Self::BadVersion(error) => error.fmt(f),
            Self::UnknownDataplane(id) => write!(f, "data plane {} does not exist", id.0),
            Self::NoMinimumConfigured { dataplane } => write!(
                f,
                "cannot upgrade data plane {}: DATAPLANE_UPGRADE_MIN_VERSION is not set, so no \
                 data plane is known to be able to receive an upgrade",
                dataplane.0
            ),
            Self::HeraldTooOld {
                dataplane,
                reported: Some(reported),
                minimum,
            } => write!(
                f,
                "data plane {} reports version {reported}, below the {minimum} an upgrade \
                 needs; upgrade it by hand first",
                dataplane.0
            ),
            Self::HeraldTooOld {
                dataplane,
                reported: None,
                minimum,
            } => write!(
                f,
                "data plane {} has reported no version, so it cannot be shown to be at {minimum} \
                 or above",
                dataplane.0
            ),
            Self::AlreadyUpgrading {
                dataplane,
                running: Some(running),
            } => write!(
                f,
                "data plane {} already has an upgrade to {running} under way",
                dataplane.0
            ),
            Self::AlreadyUpgrading {
                dataplane,
                running: None,
            } => write!(
                f,
                "data plane {} already has an upgrade under way",
                dataplane.0
            ),
            Self::Core(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for DataplaneUpgradeError {}

impl From<CoreError> for DataplaneUpgradeError {
    fn from(error: CoreError) -> Self {
        Self::Core(error)
    }
}

impl From<InvalidDataplaneUpgrade> for DataplaneUpgradeError {
    fn from(error: InvalidDataplaneUpgrade) -> Self {
        Self::Invalid(error)
    }
}

impl From<VersionError> for DataplaneUpgradeError {
    fn from(error: VersionError) -> Self {
        Self::BadVersion(error)
    }
}

enum Step {
    Create(DataPlaneId, DataplaneUpgradePayload),
    Reuse(DataPlaneId, Action),
}

pub struct DataplaneUpgradeRequestService<A, D, P>
where
    A: ActionRepository,
    D: DataPlaneRepository,
    P: PlatformPolicy,
{
    actions: ActionServiceImpl<A>,
    data_planes: D,
    policy: P,
}

impl<A, D, P> DataplaneUpgradeRequestService<A, D, P>
where
    A: ActionRepository,
    D: DataPlaneRepository,
    P: PlatformPolicy,
{
    pub fn new(actions: A, data_planes: D, policy: P) -> Self {
        Self {
            actions: ActionServiceImpl::new(actions),
            data_planes,
            policy,
        }
    }

    pub async fn request(
        &self,
        identity: Identity,
        minimum: Option<&Version>,
        request: DataplaneUpgradeRequest,
    ) -> Result<Vec<RequestedUpgrade>, DataplaneUpgradeError> {
        self.policy
            .require(identity.clone(), PlatformRight::OperateFleet)
            .await?;

        let target = Version::parse(&request.target_version)?.to_string();
        let targets = self.targets(request.dataplanes.as_deref()).await?;

        let mut payloads = Vec::with_capacity(targets.len());
        for plane in &targets {
            payloads.push(DataplaneUpgradePayload::new(
                plane.id,
                target.clone(),
                request.components.clone(),
                request.strategy,
                request.max_unavailable,
            )?);
        }

        for plane in &targets {
            guard(plane, minimum)?;
        }

        let mut steps = Vec::with_capacity(targets.len());
        for (plane, payload) in targets.iter().zip(payloads) {
            match self.open_upgrade(plane.id).await? {
                Some(open) if target_of(&open) == Some(target.as_str()) => {
                    steps.push(Step::Reuse(plane.id, open));
                }
                Some(open) => {
                    return Err(DataplaneUpgradeError::AlreadyUpgrading {
                        dataplane: plane.id,
                        running: target_of(&open).map(str::to_string),
                    });
                }
                None => steps.push(Step::Create(plane.id, payload)),
            }
        }

        let source = ActionSource::Api {
            client_id: identity.id().to_string(),
        };

        let mut requested = Vec::with_capacity(steps.len());
        for step in steps {
            requested.push(match step {
                Step::Reuse(dataplane_id, action) => RequestedUpgrade {
                    dataplane_id,
                    action,
                    already_requested: true,
                },
                Step::Create(dataplane_id, payload) => {
                    let action = self
                        .actions
                        .record_action(RecordActionCommand::for_data_plane(
                            dataplane_id,
                            ActionType::dataplane_upgrade(),
                            payload.into_action_payload(),
                            ActionVersion(PAYLOAD_VERSION),
                            source.clone(),
                        ))
                        .await?;
                    RequestedUpgrade {
                        dataplane_id,
                        action,
                        already_requested: false,
                    }
                }
            });
        }

        Ok(requested)
    }

    pub async fn list(&self, identity: Identity) -> Result<Vec<DataplaneUpgrades>, CoreError> {
        self.policy
            .require(identity, PlatformRight::ViewEstate)
            .await?;

        let mut listed = Vec::new();
        for plane in self.data_planes.list_all().await? {
            let actions = self.upgrade_actions(plane.id).await?;
            if !actions.is_empty() {
                listed.push(DataplaneUpgrades {
                    dataplane_id: plane.id,
                    actions,
                });
            }
        }

        Ok(listed)
    }

    async fn targets(
        &self,
        requested: Option<&[DataPlaneId]>,
    ) -> Result<Vec<DataPlane>, DataplaneUpgradeError> {
        let Some(ids) = requested else {
            return Ok(self.data_planes.list_all().await?);
        };

        let mut planes: Vec<DataPlane> = Vec::with_capacity(ids.len());
        for id in ids {
            if planes.iter().any(|plane| plane.id == *id) {
                continue;
            }
            let plane = self
                .data_planes
                .find_by_id(id)
                .await?
                .ok_or(DataplaneUpgradeError::UnknownDataplane(*id))?;
            planes.push(plane);
        }

        Ok(planes)
    }

    async fn open_upgrade(&self, dataplane: DataPlaneId) -> Result<Option<Action>, CoreError> {
        Ok(self
            .upgrade_actions(dataplane)
            .await?
            .into_iter()
            .rev()
            .find(|action| !is_terminal(&action.status)))
    }

    async fn upgrade_actions(&self, dataplane: DataPlaneId) -> Result<Vec<Action>, CoreError> {
        let mut found = Vec::new();
        let mut command = FetchActionsCommand::for_data_plane(dataplane, PAGE);

        loop {
            let batch = self.actions.fetch_actions(command.clone()).await?;
            let empty = batch.actions.is_empty();
            found.extend(
                batch
                    .actions
                    .into_iter()
                    .filter(|action| action.action_type == ActionType::dataplane_upgrade()),
            );

            match batch.next_cursor {
                Some(cursor) if !empty => command = command.with_cursor(cursor),
                _ => return Ok(found),
            }
        }
    }
}

fn guard(plane: &DataPlane, minimum: Option<&Version>) -> Result<(), DataplaneUpgradeError> {
    let minimum = minimum.ok_or(DataplaneUpgradeError::NoMinimumConfigured {
        dataplane: plane.id,
    })?;

    match &plane.operator_version {
        Some(reported) if reported >= minimum => Ok(()),
        reported => Err(DataplaneUpgradeError::HeraldTooOld {
            dataplane: plane.id,
            reported: reported.clone(),
            minimum: minimum.clone(),
        }),
    }
}

fn is_terminal(status: &ActionStatus) -> bool {
    matches!(
        status,
        ActionStatus::Published { .. } | ActionStatus::Failed { .. }
    )
}

fn target_of(action: &Action) -> Option<&str> {
    action.payload.data.get("target_version")?.as_str()
}

impl AetherService {
    pub async fn request_dataplane_upgrade(
        &self,
        identity: Identity,
        minimum: Option<&Version>,
        request: DataplaneUpgradeRequest,
    ) -> Result<Vec<RequestedUpgrade>, DataplaneUpgradeError> {
        aether_persistence::with_tx(
            self.pool(),
            |error| DataplaneUpgradeError::Core(aether_postgres::map_sqlx_error(error)),
            async |tx| {
                let action_repository = <Backend as RepoFor<domain::Action>>::build(&tx);
                let data_plane_repository = <Backend as RepoFor<domain::DataPlane>>::build(&tx);

                DataplaneUpgradeRequestService::new(
                    action_repository,
                    data_plane_repository,
                    PlatformRightsPolicy::new(
                        aether_postgres::platform::PostgresOperatorRepository::new(&tx),
                    ),
                )
                .request(identity, minimum, request)
                .await
            },
        )
        .await
    }

    #[transactional(action, data_plane)]
    pub async fn list_dataplane_upgrades(
        &self,
        identity: Identity,
    ) -> Result<Vec<DataplaneUpgrades>, CoreError> {
        DataplaneUpgradeRequestService::new(
            action_repository,
            data_plane_repository,
            PlatformRightsPolicy::new(aether_postgres::platform::PostgresOperatorRepository::new(
                &tx,
            )),
        )
        .list(identity)
        .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use aether_auth::User;
    use aether_domain::{
        action::{ActionBatch, ActionCursor, ActionFailureReason, ActionId, ActionScope},
        dataplane::value_objects::{
            Capacity, DataPlaneAllocation, DataPlaneMode, DeploymentResources, PlacementRequest,
            Region,
        },
        deployments::DeploymentId,
        organisation::OrganisationId,
        platform::{PlatformRight, PlatformRights},
    };
    use chrono::{DateTime, Utc};

    use super::*;

    #[derive(Clone, Default)]
    struct FakeActions(Arc<Mutex<Vec<Action>>>);

    impl FakeActions {
        fn all(&self) -> Vec<Action> {
            self.0.lock().unwrap().clone()
        }

        fn set_status(&self, id: ActionId, status: ActionStatus) {
            for action in self.0.lock().unwrap().iter_mut() {
                if action.id == id {
                    action.status = status.clone();
                }
            }
        }
    }

    impl ActionRepository for FakeActions {
        async fn append(&self, action: Action) -> Result<(), CoreError> {
            self.0.lock().unwrap().push(action);
            Ok(())
        }

        async fn get_by_id(
            &self,
            _: DeploymentId,
            _: ActionId,
        ) -> Result<Option<Action>, CoreError> {
            Ok(None)
        }

        async fn list(
            &self,
            scope: ActionScope,
            _: Option<ActionCursor>,
            _: usize,
        ) -> Result<ActionBatch, CoreError> {
            let actions = self
                .all()
                .into_iter()
                .filter(|action| match scope {
                    ActionScope::DataPlane(id) => {
                        action.dataplane_id == id && action.deployment_id.is_none()
                    }
                    ActionScope::Deployment(id) => action.deployment_id == Some(id),
                })
                .collect();
            Ok(ActionBatch {
                actions,
                next_cursor: None,
            })
        }

        async fn last_of_type(
            &self,
            _: DeploymentId,
            _: &ActionType,
        ) -> Result<Option<DateTime<Utc>>, CoreError> {
            Ok(None)
        }

        async fn claim_pending(
            &self,
            _: DataPlaneId,
            _: Vec<DeploymentId>,
            _: usize,
            _: DateTime<Utc>,
            _: DateTime<Utc>,
        ) -> Result<Vec<Action>, CoreError> {
            Ok(Vec::new())
        }

        async fn claim_dataplane_pending(
            &self,
            _: DataPlaneId,
            _: usize,
            _: DateTime<Utc>,
            _: DateTime<Utc>,
        ) -> Result<Vec<Action>, CoreError> {
            Ok(Vec::new())
        }

        async fn ack_published(
            &self,
            _: ActionScope,
            _: ActionId,
            _: DateTime<Utc>,
        ) -> Result<bool, CoreError> {
            Ok(false)
        }

        async fn ack_failed(
            &self,
            _: ActionScope,
            _: ActionId,
            _: ActionFailureReason,
            _: DateTime<Utc>,
        ) -> Result<bool, CoreError> {
            Ok(false)
        }

        async fn list_stuck(&self) -> Result<Vec<Action>, CoreError> {
            Ok(Vec::new())
        }
    }

    #[derive(Clone)]
    struct FakePlanes(Vec<DataPlane>);

    impl DataPlaneRepository for FakePlanes {
        async fn find_by_herald_subject(&self, _: &str) -> Result<Option<DataPlane>, CoreError> {
            Ok(None)
        }

        async fn find_by_id(&self, id: &DataPlaneId) -> Result<Option<DataPlane>, CoreError> {
            Ok(self.0.iter().find(|plane| plane.id == *id).cloned())
        }

        async fn find_active_shared_by_region(
            &self,
            _: &Region,
        ) -> Result<Vec<DataPlane>, CoreError> {
            Ok(Vec::new())
        }

        async fn find_available(
            &self,
            _: PlacementRequest,
        ) -> Result<Option<DataPlane>, CoreError> {
            Ok(None)
        }

        async fn region_is_served(&self, _: &Region) -> Result<bool, CoreError> {
            Ok(false)
        }

        async fn region_blocked_by_deployment_count(
            &self,
            _: &Region,
            _: DataPlaneMode,
            _: DeploymentResources,
        ) -> Result<bool, CoreError> {
            Ok(false)
        }

        async fn find_dedicated_for_organisation(
            &self,
            _: &OrganisationId,
            _: &Region,
        ) -> Result<Option<DataPlane>, CoreError> {
            Ok(None)
        }

        async fn list_all(&self) -> Result<Vec<DataPlane>, CoreError> {
            Ok(self.0.clone())
        }

        async fn current_load(&self, _: &DataPlaneId) -> Result<u32, CoreError> {
            Ok(0)
        }

        async fn save(&self, _: &DataPlane) -> Result<(), CoreError> {
            Ok(())
        }

        async fn touch_last_seen(
            &self,
            _: &DataPlaneId,
            _: DateTime<Utc>,
            _: Option<Version>,
            _: Option<String>,
        ) -> Result<bool, CoreError> {
            Ok(false)
        }
    }

    struct Holding(PlatformRights);

    impl PlatformPolicy for Holding {
        async fn require(&self, _: Identity, right: PlatformRight) -> Result<(), CoreError> {
            if self.0.holds(right) {
                return Ok(());
            }
            Err(CoreError::MissingPlatformRight {
                right: right.to_string(),
            })
        }

        async fn rights_of(&self, _: Identity) -> Result<PlatformRights, CoreError> {
            Ok(self.0.clone())
        }
    }

    fn identity() -> Identity {
        Identity::User(User {
            id: "operator-1".to_string(),
            username: "operator".to_string(),
            email: None,
            name: None,
            roles: vec![],
        })
    }

    fn plane(version: Option<Version>) -> DataPlane {
        let mut plane = DataPlane::new(
            DataPlaneAllocation::Shared,
            Region::new("fr-par"),
            Capacity::new(4_000, 8_192, 100).unwrap(),
        );
        plane.operator_version = version;
        plane
    }

    fn v(minor: u64) -> Version {
        Version::new(26, minor, 0)
    }

    fn rights(list: &[PlatformRight]) -> PlatformRights {
        PlatformRights::of(list.iter().copied())
    }

    fn service(
        planes: Vec<DataPlane>,
        held: &[PlatformRight],
    ) -> (
        DataplaneUpgradeRequestService<FakeActions, FakePlanes, Holding>,
        FakeActions,
    ) {
        let actions = FakeActions::default();
        let service = DataplaneUpgradeRequestService::new(
            actions.clone(),
            FakePlanes(planes),
            Holding(rights(held)),
        );
        (service, actions)
    }

    fn upgrade_to(target: &str, ids: Option<Vec<DataPlaneId>>) -> DataplaneUpgradeRequest {
        DataplaneUpgradeRequest {
            target_version: target.to_string(),
            dataplanes: ids,
            components: vec![UpgradeComponent::All],
            strategy: UpgradeStrategy::Rolling,
            max_unavailable: 1,
        }
    }

    const OPERATE: &[PlatformRight] = &[PlatformRight::OperateFleet, PlatformRight::ViewEstate];

    #[tokio::test]
    async fn without_operate_fleet_nothing_is_created() {
        let target = plane(Some(v(1)));
        let (service, actions) = service(vec![target], &[PlatformRight::ViewEstate]);

        let result = service
            .request(identity(), Some(&v(0)), upgrade_to("26.2.0", None))
            .await;

        assert!(matches!(
            result,
            Err(DataplaneUpgradeError::Core(
                CoreError::MissingPlatformRight { .. }
            ))
        ));
        assert!(actions.all().is_empty());
    }

    #[tokio::test]
    async fn an_invalid_request_creates_nothing() {
        let (service, actions) = service(vec![plane(Some(v(1)))], OPERATE);
        let minimum = v(0);

        let mut no_components = upgrade_to("26.2.0", None);
        no_components.components = vec![];
        let mut nothing_may_go_down = upgrade_to("26.2.0", None);
        nothing_may_go_down.max_unavailable = 0;

        for request in [
            no_components,
            nothing_may_go_down,
            upgrade_to("latest", None),
        ] {
            let result = service.request(identity(), Some(&minimum), request).await;
            assert!(matches!(
                result,
                Err(DataplaneUpgradeError::Invalid(_) | DataplaneUpgradeError::BadVersion(_))
            ));
        }
        assert!(actions.all().is_empty());
    }

    #[tokio::test]
    async fn an_unknown_data_plane_refuses_the_whole_request() {
        let known = plane(Some(v(1)));
        let unknown = DataPlaneId(uuid::Uuid::new_v4());
        let (service, actions) = service(vec![known.clone()], OPERATE);

        let result = service
            .request(
                identity(),
                Some(&v(0)),
                upgrade_to("26.2.0", Some(vec![known.id, unknown])),
            )
            .await;

        assert!(matches!(
            result,
            Err(DataplaneUpgradeError::UnknownDataplane(id)) if id == unknown
        ));
        assert!(actions.all().is_empty());
    }

    #[tokio::test]
    async fn one_action_is_created_per_targeted_data_plane() {
        let first = plane(Some(v(1)));
        let second = plane(Some(v(1)));
        let untouched = plane(Some(v(1)));
        let (service, actions) = service(
            vec![first.clone(), second.clone(), untouched.clone()],
            OPERATE,
        );

        let requested = service
            .request(
                identity(),
                Some(&v(0)),
                upgrade_to("26.2.0", Some(vec![first.id, second.id, first.id])),
            )
            .await
            .unwrap();

        assert_eq!(requested.len(), 2);
        assert!(requested.iter().all(|upgrade| !upgrade.already_requested));
        let recorded = actions.all();
        assert_eq!(recorded.len(), 2);
        assert!(recorded.iter().all(|action| {
            action.deployment_id.is_none()
                && action.action_type == ActionType::dataplane_upgrade()
                && action.version == ActionVersion(PAYLOAD_VERSION)
                && action.payload.data["target_version"] == "26.2.0"
                && action.dataplane_id != untouched.id
        }));
    }

    #[tokio::test]
    async fn every_data_plane_is_targeted_when_none_are_named() {
        let (service, actions) = service(vec![plane(Some(v(1))), plane(Some(v(1)))], OPERATE);

        service
            .request(identity(), Some(&v(0)), upgrade_to("26.2.0", None))
            .await
            .unwrap();

        assert_eq!(actions.all().len(), 2);
    }

    #[tokio::test]
    async fn the_guard_fails_closed_without_a_minimum() {
        let target = plane(Some(v(5)));
        let (service, actions) = service(vec![target.clone()], OPERATE);

        let result = service
            .request(identity(), None, upgrade_to("26.9.0", None))
            .await;

        assert!(matches!(
            result,
            Err(DataplaneUpgradeError::NoMinimumConfigured { dataplane }) if dataplane == target.id
        ));
        assert!(actions.all().is_empty());
    }

    #[tokio::test]
    async fn the_guard_refuses_a_data_plane_reporting_no_version() {
        let target = plane(None);
        let (service, actions) = service(vec![target.clone()], OPERATE);

        let result = service
            .request(identity(), Some(&v(1)), upgrade_to("26.9.0", None))
            .await;

        assert!(matches!(
            result,
            Err(DataplaneUpgradeError::HeraldTooOld { dataplane, reported: None, .. })
                if dataplane == target.id
        ));
        assert!(actions.all().is_empty());
    }

    #[tokio::test]
    async fn the_guard_refuses_below_the_minimum_and_passes_at_or_above_it() {
        let old = plane(Some(v(1)));
        let (service, actions) = service(vec![old], OPERATE);
        let refused = service
            .request(identity(), Some(&v(2)), upgrade_to("26.9.0", None))
            .await;
        assert!(matches!(
            refused,
            Err(DataplaneUpgradeError::HeraldTooOld { .. })
        ));
        assert!(actions.all().is_empty());

        for reported in [v(2), v(3)] {
            let (service, actions) = service_with(reported);
            service
                .request(identity(), Some(&v(2)), upgrade_to("26.9.0", None))
                .await
                .unwrap();
            assert_eq!(actions.all().len(), 1);
        }
    }

    fn service_with(
        reported: Version,
    ) -> (
        DataplaneUpgradeRequestService<FakeActions, FakePlanes, Holding>,
        FakeActions,
    ) {
        service(vec![plane(Some(reported))], OPERATE)
    }

    #[tokio::test]
    async fn one_old_data_plane_refuses_the_whole_request() {
        let (service, actions) = service(vec![plane(Some(v(5))), plane(Some(v(1)))], OPERATE);

        let result = service
            .request(identity(), Some(&v(2)), upgrade_to("26.9.0", None))
            .await;

        assert!(matches!(
            result,
            Err(DataplaneUpgradeError::HeraldTooOld { .. })
        ));
        assert!(actions.all().is_empty());
    }

    #[tokio::test]
    async fn a_second_upgrade_to_another_version_is_refused_and_to_the_same_returns_the_first() {
        let target = plane(Some(v(5)));
        let (service, actions) = service(vec![target.clone()], OPERATE);
        let first = service
            .request(identity(), Some(&v(2)), upgrade_to("26.9.0", None))
            .await
            .unwrap();

        let other = service
            .request(identity(), Some(&v(2)), upgrade_to("26.10.0", None))
            .await;
        let same = service
            .request(identity(), Some(&v(2)), upgrade_to("26.9.0", None))
            .await
            .unwrap();

        assert!(matches!(
            other,
            Err(DataplaneUpgradeError::AlreadyUpgrading { running: Some(running), .. })
                if running == "26.9.0"
        ));
        assert_eq!(same.len(), 1);
        assert!(same[0].already_requested);
        assert_eq!(same[0].action.id, first[0].action.id);
        assert_eq!(actions.all().len(), 1);
    }

    #[tokio::test]
    async fn a_finished_upgrade_no_longer_blocks_another() {
        let target = plane(Some(v(5)));
        let (service, actions) = service(vec![target], OPERATE);
        let first = service
            .request(identity(), Some(&v(2)), upgrade_to("26.9.0", None))
            .await
            .unwrap();
        actions.set_status(
            first[0].action.id,
            ActionStatus::Published { at: Utc::now() },
        );

        let next = service
            .request(identity(), Some(&v(2)), upgrade_to("26.10.0", None))
            .await
            .unwrap();

        assert!(!next[0].already_requested);
        assert_eq!(actions.all().len(), 2);
    }

    #[tokio::test]
    async fn a_conflict_on_one_data_plane_creates_nothing_for_the_others() {
        let busy = plane(Some(v(5)));
        let idle = plane(Some(v(5)));
        let (service, actions) = service(vec![busy.clone(), idle.clone()], OPERATE);
        service
            .request(
                identity(),
                Some(&v(2)),
                upgrade_to("26.9.0", Some(vec![busy.id])),
            )
            .await
            .unwrap();

        let result = service
            .request(identity(), Some(&v(2)), upgrade_to("26.10.0", None))
            .await;

        assert!(matches!(
            result,
            Err(DataplaneUpgradeError::AlreadyUpgrading { .. })
        ));
        assert_eq!(actions.all().len(), 1);
    }

    #[tokio::test]
    async fn listing_returns_the_upgrade_actions_per_data_plane_and_needs_view_estate() {
        let with = plane(Some(v(5)));
        let without = plane(Some(v(5)));
        let (service, _) = service(vec![with.clone(), without], OPERATE);
        service
            .request(
                identity(),
                Some(&v(2)),
                upgrade_to("26.9.0", Some(vec![with.id])),
            )
            .await
            .unwrap();

        let listed = service.list(identity()).await.unwrap();

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].dataplane_id, with.id);
        assert_eq!(listed[0].actions.len(), 1);
        assert_eq!(listed[0].actions[0].status, ActionStatus::Pending);

        let (blind, _) = service_without_view();
        assert!(matches!(
            blind.list(identity()).await,
            Err(CoreError::MissingPlatformRight { .. })
        ));
    }

    fn service_without_view() -> (
        DataplaneUpgradeRequestService<FakeActions, FakePlanes, Holding>,
        FakeActions,
    ) {
        service(vec![], &[PlatformRight::OperateFleet])
    }
}
