use aether_auth::Identity;
use aether_domain::{
    CoreError,
    action::{
        ActionPayload, ActionSource, ActionTarget, ActionType, ActionVersion, TargetKind,
        commands::RecordActionCommand, ports::ActionService, service::ActionServiceImpl,
    },
    logs::{
        LogSession,
        commands::{PushLogLinesCommand, ReadLogsCommand},
        ports::{LogRelay, LogService},
        service::{AcceptedLogRead, LogServiceImpl, log_request_payload},
    },
};
use aether_macros::transactional;

use crate::{
    AetherService,
    infrastructure::{
        logs::ChannelLogStream,
        role::{PostgresRoleRepository, RolePermissionProvider},
    },
    policy::AetherPolicy,
};

impl LogService for AetherService {
    type Stream = ChannelLogStream;

    async fn read_logs(
        &self,
        identity: Identity,
        command: ReadLogsCommand,
    ) -> Result<(LogSession, ChannelLogStream), CoreError> {
        let accepted = self.accept_log_read(identity, command).await?;
        let stream = self.log_relay().open(accepted.session.clone()).await?;

        Ok((accepted.session, stream))
    }

    /// Touches no database. The session id only ever travelled to the data
    /// plane holding the deployment, and the caller has already been checked
    /// to be a data plane agent, so there is nothing to look up and nothing
    /// to write.
    async fn push_log_lines(
        &self,
        identity: Identity,
        command: PushLogLinesCommand,
    ) -> Result<(), CoreError> {
        // The same rule as claim, ack, heartbeat and outcome: only Herald
        // speaks for a data plane. A caller able to forge this could feed a
        // customer's screen lines that never happened.
        if !identity.username().contains("herald-service") {
            return Err(CoreError::PermissionDenied {
                reason: "only herald can send logs".to_string(),
            });
        }

        if !command.lines.is_empty() {
            self.log_relay()
                .push(command.session_id, command.lines)
                .await?;
        }

        if command.done {
            self.log_relay().close(command.session_id).await?;
        }

        Ok(())
    }
}

impl AetherService {
    #[transactional(deployment, audit, user, action)]
    async fn accept_log_read(
        &self,
        identity: Identity,
        command: ReadLogsCommand,
    ) -> Result<AcceptedLogRead, CoreError> {
        let accepted = LogServiceImpl::new(
            deployment_repository,
            audit_repository,
            user_repository,
            AetherPolicy::new(RolePermissionProvider::new(PostgresRoleRepository::new(
                &tx,
            ))),
            self.log_relay().clone(),
        )
        .accept_read(identity, command)
        .await?;

        // In the same transaction as the audit entry: an action that outlived
        // a rolled-back entry would have a data plane send logs that nothing
        // recorded anyone asking for.
        ActionServiceImpl::new(action_repository)
            .record_action(RecordActionCommand::new(
                accepted.deployment.id,
                accepted.deployment.dataplane_id,
                ActionType("deployment.logs".to_string()),
                ActionTarget {
                    kind: TargetKind::Deployment,
                    id: accepted.deployment.id.0,
                },
                ActionPayload {
                    data: log_request_payload(&accepted),
                },
                ActionVersion(1),
                ActionSource::System,
            ))
            .await?;

        Ok(accepted)
    }
}
