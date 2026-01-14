use crate::domain::action::{
    ActionConstraints, ActionCursor, ActionPayload, ActionSource, ActionTarget, ActionType,
    ActionVersion,
};
use crate::domain::deployments::DeploymentId;

#[derive(Debug, Clone)]
pub struct RecordActionCommand {
    pub action_type: ActionType,
    pub target: ActionTarget,
    pub payload: ActionPayload,
    pub version: ActionVersion,
    pub source: ActionSource,
    pub constraints: ActionConstraints,
}

impl RecordActionCommand {
    pub fn new(
        action_type: ActionType,
        target: ActionTarget,
        payload: ActionPayload,
        version: ActionVersion,
        source: ActionSource,
    ) -> Self {
        Self {
            action_type,
            target,
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
    pub deployment_id: DeploymentId,
    pub cursor: Option<ActionCursor>,
    pub limit: usize,
}

impl FetchActionsCommand {
    pub fn new(deployment_id: DeploymentId, limit: usize) -> Self {
        Self {
            deployment_id,
            cursor: None,
            limit,
        }
    }

    pub fn with_cursor(mut self, cursor: ActionCursor) -> Self {
        self.cursor = Some(cursor);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    use crate::domain::action::{
        ActionPayload, ActionSource, ActionTarget, ActionType, ActionVersion, TargetKind,
    };

    #[test]
    fn record_action_command_defaults_constraints() {
        let command = RecordActionCommand::new(
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
    fn fetch_actions_command_sets_cursor() {
        let deployment_id = DeploymentId(Uuid::new_v4());
        let command =
            FetchActionsCommand::new(deployment_id, 50).with_cursor(ActionCursor::new("cursor-1"));

        assert_eq!(command.deployment_id, deployment_id);
        assert_eq!(command.limit, 50);
        assert_eq!(command.cursor, Some(ActionCursor::new("cursor-1")));
    }
}
