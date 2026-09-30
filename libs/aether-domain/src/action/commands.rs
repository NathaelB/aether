use crate::action::{
    ActionConstraints, ActionCursor, ActionFailureReason, ActionId, ActionPayload, ActionScope,
    ActionSource, ActionTarget, ActionType, ActionVersion, TargetKind,
};
use crate::{dataplane::value_objects::DataPlaneId, deployments::DeploymentId};

#[derive(Debug, Clone)]
pub struct RecordActionCommand {
    pub deployment_id: Option<DeploymentId>,
    pub dataplane_id: DataPlaneId,
    pub action_type: ActionType,
    pub target: ActionTarget,
    pub payload: ActionPayload,
    pub version: ActionVersion,
    pub source: ActionSource,
    pub constraints: ActionConstraints,
}

impl RecordActionCommand {
    pub fn new(
        deployment_id: DeploymentId,
        dataplane_id: DataPlaneId,
        action_type: ActionType,
        target: ActionTarget,
        payload: ActionPayload,
        version: ActionVersion,
        source: ActionSource,
    ) -> Self {
        Self {
            deployment_id: Some(deployment_id),
            dataplane_id,
            action_type,
            target,
            payload,
            version,
            source,
            constraints: ActionConstraints::default(),
        }
    }

    pub fn for_data_plane(
        dataplane_id: DataPlaneId,
        action_type: ActionType,
        payload: ActionPayload,
        version: ActionVersion,
        source: ActionSource,
    ) -> Self {
        Self {
            deployment_id: None,
            dataplane_id,
            action_type,
            target: ActionTarget {
                kind: TargetKind::DataPlane,
                id: dataplane_id.0,
            },
            payload,
            version,
            source,
            constraints: ActionConstraints::default(),
        }
    }

    pub fn with_constraints(mut self, constraints: ActionConstraints) -> Self {
        self.constraints = constraints;
        self
    }
}

#[derive(Debug, Clone)]
pub struct FetchActionsCommand {
    pub scope: ActionScope,
    pub cursor: Option<ActionCursor>,
    pub limit: usize,
}

impl FetchActionsCommand {
    pub fn new(deployment_id: DeploymentId, limit: usize) -> Self {
        Self {
            scope: ActionScope::Deployment(deployment_id),
            cursor: None,
            limit,
        }
    }

    pub fn for_data_plane(dataplane_id: DataPlaneId, limit: usize) -> Self {
        Self {
            scope: ActionScope::DataPlane(dataplane_id),
            cursor: None,
            limit,
        }
    }

    pub fn with_cursor(mut self, cursor: ActionCursor) -> Self {
        self.cursor = Some(cursor);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ClaimActionsCommand {
    pub dataplane_id: DataPlaneId,
    /// This Herald's shard, already resolved to the deployments it owns.
    pub deployment_ids: Vec<DeploymentId>,
    pub max: usize,
    pub lease_seconds: i64,
    /// Whether actions addressed to the data plane itself are claimed too.
    /// Off unless the caller asks, so a Herald that cannot route them never
    /// receives one.
    pub include_dataplane_actions: bool,
}

/// A single failed action reported by a caller acknowledging its outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckFailure {
    pub action_id: ActionId,
    pub reason: ActionFailureReason,
}

#[derive(Debug, Clone)]
pub struct AckActionsCommand {
    pub dataplane_id: DataPlaneId,
    pub scope: ActionScope,
    pub published: Vec<ActionId>,
    pub failed: Vec<AckFailure>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    use crate::action::{
        ActionPayload, ActionSource, ActionTarget, ActionType, ActionVersion, TargetKind,
    };
    use crate::dataplane::value_objects::DataPlaneId;

    #[test]
    fn record_action_command_defaults_constraints() {
        let command = RecordActionCommand::new(
            DeploymentId(Uuid::new_v4()),
            DataPlaneId(Uuid::new_v4()),
            ActionType("deployment.create".to_string()),
            ActionTarget {
                kind: TargetKind::Deployment,
                id: Uuid::new_v4(),
            },
            ActionPayload {
                data: json!({"id": "dep-1"}),
            },
            ActionVersion(1),
            ActionSource::Api {
                client_id: "test-client".to_string(),
            },
        );

        assert_eq!(command.constraints, ActionConstraints::default());
    }

    #[test]
    fn record_action_command_allows_constraints_override() {
        let constraints = ActionConstraints {
            not_after: Some(Utc::now()),
            priority: Some(2),
        };

        let command = RecordActionCommand::new(
            DeploymentId(Uuid::new_v4()),
            DataPlaneId(Uuid::new_v4()),
            ActionType("deployment.create".to_string()),
            ActionTarget {
                kind: TargetKind::Deployment,
                id: Uuid::new_v4(),
            },
            ActionPayload {
                data: json!({"id": "dep-1"}),
            },
            ActionVersion(1),
            ActionSource::System,
        )
        .with_constraints(constraints.clone());

        assert_eq!(command.constraints, constraints);
    }

    #[test]
    fn data_plane_action_has_no_deployment_and_targets_the_data_plane() {
        let dataplane_id = DataPlaneId(Uuid::new_v4());

        let command = RecordActionCommand::for_data_plane(
            dataplane_id,
            ActionType::dataplane_upgrade(),
            ActionPayload { data: json!({}) },
            ActionVersion(1),
            ActionSource::System,
        );

        assert_eq!(command.deployment_id, None);
        assert_eq!(command.target.kind, TargetKind::DataPlane);
        assert_eq!(command.target.id, dataplane_id.0);
    }

    #[test]
    fn fetch_actions_command_sets_cursor() {
        let deployment_id = DeploymentId(Uuid::new_v4());
        let command =
            FetchActionsCommand::new(deployment_id, 50).with_cursor(ActionCursor::new("cursor-1"));

        assert_eq!(command.scope, ActionScope::Deployment(deployment_id));
        assert_eq!(command.limit, 50);
        assert_eq!(command.cursor, Some(ActionCursor::new("cursor-1")));
    }
}
