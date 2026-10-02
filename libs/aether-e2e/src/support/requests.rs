use std::sync::{Arc, Mutex};

use aether_domain::CoreError;
use aether_domain::action::{
    Action, ActionBatch, ActionCursor, ActionFailureReason, ActionId, ActionScope, ActionStatus,
    ActionType, ports::ActionRepository,
};
use aether_domain::dataplane::entities::DataPlane;
use aether_domain::dataplane::ports::DataPlaneRepository;
use aether_domain::dataplane::value_objects::{
    Capacity, DataPlaneAllocation, DataPlaneId, DataPlaneMode, DeploymentResources,
    PlacementRequest, Region,
};
use aether_domain::deployments::DeploymentId;
use aether_domain::organisation::OrganisationId;
use aether_domain::version::Version;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Default)]
pub struct InMemoryActions(Arc<Mutex<Vec<Action>>>);

impl InMemoryActions {
    pub fn all(&self) -> Vec<Action> {
        self.0.lock().expect("not poisoned").clone()
    }

    pub fn addressed_to(&self, plane: DataPlaneId) -> Vec<Action> {
        self.all()
            .into_iter()
            .filter(|action| action.dataplane_id == plane)
            .collect()
    }

    pub fn set_status(&self, plane: DataPlaneId, status: &ActionStatus) {
        for action in self.0.lock().expect("not poisoned").iter_mut() {
            if action.dataplane_id == plane {
                action.status = status.clone();
            }
        }
    }
}

pub fn published(at: DateTime<Utc>) -> ActionStatus {
    ActionStatus::Published { at }
}

pub fn failed(at: DateTime<Utc>) -> ActionStatus {
    ActionStatus::Failed {
        reason: ActionFailureReason::PublishFailed,
        at,
    }
}

pub fn is_dataplane_upgrade(action: &Action) -> bool {
    action.action_type == ActionType::dataplane_upgrade()
}

impl ActionRepository for InMemoryActions {
    async fn append(&self, action: Action) -> Result<(), CoreError> {
        self.0.lock().expect("not poisoned").push(action);
        Ok(())
    }

    async fn get_by_id(&self, _: DeploymentId, _: ActionId) -> Result<Option<Action>, CoreError> {
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

#[derive(Debug, Clone, Default)]
pub struct InMemoryDataPlanes(Arc<Mutex<Vec<DataPlane>>>);

impl InMemoryDataPlanes {
    pub fn register(&self, reported: Option<Version>) -> DataPlaneId {
        let mut plane = DataPlane::new(
            DataPlaneAllocation::Shared,
            Region::new("fr-par"),
            Capacity::new(4_000, 8_192, 100).expect("a non-zero capacity"),
        );
        plane.operator_version = reported;
        let id = plane.id;
        self.0.lock().expect("not poisoned").push(plane);
        id
    }
}

impl DataPlaneRepository for InMemoryDataPlanes {
    async fn find_by_herald_subject(&self, _: &str) -> Result<Option<DataPlane>, CoreError> {
        Ok(None)
    }

    async fn find_by_id(&self, id: &DataPlaneId) -> Result<Option<DataPlane>, CoreError> {
        Ok(self
            .0
            .lock()
            .expect("not poisoned")
            .iter()
            .find(|plane| plane.id == *id)
            .cloned())
    }

    async fn find_active_shared_by_region(&self, _: &Region) -> Result<Vec<DataPlane>, CoreError> {
        Ok(Vec::new())
    }

    async fn find_available(&self, _: PlacementRequest) -> Result<Option<DataPlane>, CoreError> {
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
        Ok(self.0.lock().expect("not poisoned").clone())
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
