use crate::domain::entities::action::{AckFailure, Action, ActionEvent, ActionFailureReason};
use crate::domain::entities::dataplane::DataPlaneId;
use crate::domain::entities::deployment::DeploymentId;
use crate::domain::entities::logs::{LOG_ACTION_TYPE, LogSessionId, LogStreamRequest};
use crate::domain::entities::shard::ShardConfig;
use crate::domain::error::HeraldError;
use crate::domain::log_session::run_log_session;
use crate::domain::ports::{
    ControlPlaneRepository, HeraldService, MessageBusRepository, OutcomeInboxRepository,
    PodLogSource, UsageSource,
};
use crate::domain::usage_collector::UsageCollector;
use chrono::Utc;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::warn;

/// How many outcomes one cycle carries.
///
/// Bounded so a backlog cannot turn a sync cycle into an unbounded one: what
/// is not carried this cycle is carried by the next, fifteen seconds later.
const OUTCOMES_PER_CYCLE: usize = 64;

pub struct HeraldServiceImpl<CP, MB, OI, US, PL>
where
    CP: ControlPlaneRepository,
    MB: MessageBusRepository,
    OI: OutcomeInboxRepository,
    US: UsageSource,
    PL: PodLogSource,
{
    control_plane: Arc<CP>,
    message_bus: Arc<MB>,
    outcomes: Arc<OI>,
    usage_source: Arc<US>,
    pod_logs: Arc<PL>,
    /// The only state Herald keeps between cycles, and the only thing a
    /// restart loses.
    usage: Mutex<UsageCollector>,
    /// The sessions currently being followed.
    ///
    /// Ids only, never lines: what this holds is the answer to "am I already
    /// doing this", not a copy of anybody's logs.
    log_sessions: Arc<Mutex<HashSet<LogSessionId>>>,
    dataplane_id: DataPlaneId,
    shard_config: ShardConfig,
}

impl<CP, MB, OI, US, PL> HeraldServiceImpl<CP, MB, OI, US, PL>
where
    CP: ControlPlaneRepository + 'static,
    MB: MessageBusRepository,
    OI: OutcomeInboxRepository,
    US: UsageSource,
    PL: PodLogSource + 'static,
{
    pub fn new(
        control_plane: Arc<CP>,
        message_bus: Arc<MB>,
        outcomes: Arc<OI>,
        usage_source: Arc<US>,
        pod_logs: Arc<PL>,
        dataplane_id: DataPlaneId,
        shard_config: ShardConfig,
    ) -> Self {
        Self {
            control_plane,
            message_bus,
            outcomes,
            usage_source,
            pod_logs,
            usage: Mutex::new(UsageCollector::default()),
            log_sessions: Arc::new(Mutex::new(HashSet::new())),
            dataplane_id,
            shard_config,
        }
    }

    /// Starts following a deployment's pods for one session.
    ///
    /// Returns as soon as the session is running: a session lives for minutes,
    /// and the sync cycle it was claimed in must not.
    async fn begin_log_session(&self, action: &Action) -> Result<(), HeraldError> {
        let request = LogStreamRequest::try_from(&action.payload)?;
        let session_id = request.session_id;

        // Delivery is at-least-once, so this action can arrive twice. A second
        // stream for the same session would double every line on somebody's
        // screen, which reads as the instance having done everything twice.
        if !self.log_sessions.lock().await.insert(session_id) {
            return Ok(());
        }

        let control_plane = Arc::clone(&self.control_plane);
        let pod_logs = Arc::clone(&self.pod_logs);
        let sessions = Arc::clone(&self.log_sessions);

        tokio::spawn(async move {
            run_log_session(control_plane, pod_logs, request).await;
            sessions.lock().await.remove(&session_id);
        });

        Ok(())
    }

    /// Carries whatever other components in this data plane observed.
    ///
    /// Best-effort, like the heartbeat: a report that cannot be delivered
    /// leaves a status stale, which is the situation that already existed.
    /// Failing the cycle over it would stop deployments being claimed, which
    /// is worse than the problem.
    async fn carry_outcomes(&self) {
        let reports = match self.outcomes.drain(OUTCOMES_PER_CYCLE).await {
            Ok(reports) => reports,
            Err(err) => {
                warn!(%err, "failed to read the outcome queue");
                return;
            }
        };

        for report in reports {
            if let Err(err) = self
                .control_plane
                .report_outcome(&self.dataplane_id, &report)
                .await
            {
                warn!(
                    %err,
                    deployment_id = %report.deployment_id,
                    "failed to report a deployment outcome"
                );
            }
        }
    }

    /// Claims pending actions for a single deployment, publishes each to the
    /// message bus, and acknowledges the whole batch with the control plane
    /// in a single call.
    ///
    /// A per-action publish failure does not abort the batch: it is recorded
    /// in the ack's `failed` list (with `PublishFailed`) instead, so the
    /// control plane can move that action to a terminal `Failed` state
    /// rather than leaving it leased until it silently expires and gets
    /// republished forever. This is the mechanism C3's "publish first, then
    /// ack" delivery semantics depend on.
    ///
    /// Acking is best-effort: a failure to ack is logged and swallowed,
    /// never propagated. The control plane's ack is idempotent (it only
    /// transitions actions that are still leased), so a lost ack is safe by
    /// design — the lease simply expires and the action is reclaimed.
    async fn claim_publish_and_ack(&self, deployment_id: &DeploymentId) -> Result<(), HeraldError> {
        let actions = self
            .control_plane
            .claim_actions(&self.dataplane_id, deployment_id)
            .await?;

        if actions.is_empty() {
            return Ok(());
        }

        let mut published = Vec::with_capacity(actions.len());
        let mut failed = Vec::new();

        for action in actions {
            let action_id = action.id;

            // Handled here rather than published onward. Every other action is
            // work for another component in this data plane; this one is work
            // for Herald, which is the only process holding credentials for
            // both the cluster and the control plane.
            if action.action_type == LOG_ACTION_TYPE {
                match self.begin_log_session(&action).await {
                    // "Published" in the ack's vocabulary means Herald has
                    // taken responsibility for the action, which it has.
                    Ok(()) => published.push(action_id),
                    Err(err) => {
                        warn!(
                            %deployment_id, %action_id, error = %err,
                            "unusable log request, marking as failed"
                        );
                        failed.push(AckFailure {
                            action_id,
                            reason: ActionFailureReason::InvalidPayload,
                        });
                    }
                }
                continue;
            }

            let event: ActionEvent = match action.try_into() {
                Ok(event) => event,
                Err(err) => {
                    warn!(
                        %deployment_id, %action_id, error = %err,
                        "invalid action, marking as failed"
                    );
                    failed.push(AckFailure {
                        action_id,
                        reason: ActionFailureReason::InvalidPayload,
                    });
                    continue;
                }
            };

            match self.message_bus.publish(event).await {
                Ok(()) => published.push(action_id),
                Err(err) => {
                    warn!(
                        %deployment_id, %action_id, error = %err,
                        "failed to publish action event"
                    );
                    failed.push(AckFailure {
                        action_id,
                        reason: ActionFailureReason::PublishFailed,
                    });
                }
            }
        }

        if let Err(err) = self
            .control_plane
            .ack_actions(&self.dataplane_id, deployment_id, published, failed)
            .await
        {
            warn!(
                %deployment_id, error = %err,
                "failed to ack claimed actions; unacked actions will be reclaimed once their lease expires"
            );
        }

        Ok(())
    }
}

impl<CP, MB, OI, US, PL> HeraldService for HeraldServiceImpl<CP, MB, OI, US, PL>
where
    CP: ControlPlaneRepository + 'static,
    MB: MessageBusRepository,
    OI: OutcomeInboxRepository,
    US: UsageSource,
    PL: PodLogSource + 'static,
{
    async fn sync_all_deployments(&self) -> Result<(), HeraldError> {
        // Reported before the work, not after: a cycle that fails partway
        // still proves this data plane is alive, and reporting only on success
        // would drain a data plane for a reason that has nothing to do with
        // whether it is reachable.
        //
        // Best-effort on purpose. A failed heartbeat costs at most one missed
        // window, and a control plane that cannot take it will not serve the
        // list either -- failing here would replace a useful error with a
        // useless one.
        if let Err(err) = self.control_plane.send_heartbeat(&self.dataplane_id).await {
            warn!(%err, "failed to report data plane heartbeat");
        }

        // Before claiming, so a deletion reported last cycle is recorded before
        // this cycle lists deployments -- otherwise a deployment that is gone
        // is claimed once more for no reason.
        self.carry_outcomes().await;

        let deployments = self
            .control_plane
            .list_deployments(&self.dataplane_id)
            .await?;

        for deployment in deployments {
            if !self.shard_config.owns_deployment(&deployment.id) {
                continue;
            }

            self.claim_publish_and_ack(&deployment.id).await?;
        }

        Ok(())
    }

    async fn process_deployment(&self, deployment_id: &DeploymentId) -> Result<(), HeraldError> {
        if !self.shard_config.owns_deployment(deployment_id) {
            return Ok(());
        }

        self.claim_publish_and_ack(deployment_id).await
    }

    async fn collect_usage(&self) -> Result<(), HeraldError> {
        let deployments = self
            .control_plane
            .list_deployments(&self.dataplane_id)
            .await?;

        let owned: Vec<_> = deployments
            .into_iter()
            .filter(|deployment| self.shard_config.owns_deployment(&deployment.id))
            .collect();

        let mut usage = self.usage.lock().await;
        usage.retain(&owned.iter().map(|d| d.id.clone()).collect());

        for deployment in &owned {
            let Some(target) = deployment.usage_target() else {
                continue;
            };

            match self.usage_source.sample(&target).await {
                Ok(Some(sample)) => usage.observe(&deployment.id, sample),
                // The product exposes nothing Herald can count. Reporting a
                // zero here would be an assertion about a deployment nobody
                // measured.
                Ok(None) => continue,
                Err(err) => {
                    // Nothing is recorded on purpose. The minutes this read
                    // would have covered stay absent upstream, which is the
                    // only honest way to say the instance went quiet.
                    warn!(
                        %err,
                        deployment_id = %deployment.id,
                        "could not read usage from the instance; its buckets will be missing, not zero"
                    );
                    continue;
                }
            }
        }

        usage.evict(Utc::now());

        let batches: Vec<_> = owned
            .iter()
            .map(|deployment| (deployment.id.clone(), usage.points(&deployment.id)))
            .filter(|(_, points)| !points.is_empty())
            .collect();

        // The lock is released before anything goes over the network: the
        // buckets are already decided, and holding it through a slow control
        // plane would stall the next tick's readings behind it.
        drop(usage);

        for (deployment_id, points) in batches {
            if let Err(err) = self
                .control_plane
                .report_usage(&deployment_id, &points)
                .await
            {
                // Left in the window rather than dropped. The next cycle sends
                // the same buckets again, and the control plane's write
                // overwrites, so a failure here costs nothing but a delay.
                warn!(
                    %err,
                    %deployment_id,
                    "failed to report usage buckets; they will be sent again next cycle"
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::action::{AckOutcome, Action, ActionId};
    use crate::domain::entities::deployment::Deployment;
    use crate::domain::entities::deployment::DeploymentKind;
    use crate::domain::entities::outcome::DeploymentOutcomeReport;
    use crate::domain::entities::usage::{CounterSample, UsageMetric};
    use crate::domain::ports::{
        MockControlPlaneRepository, MockMessageBusRepository, MockOutcomeInboxRepository,
        MockPodLogSource, MockUsageSource,
    };
    use chrono::{DateTime, Duration, Utc};
    use serde_json::json;
    use uuid::Uuid;

    fn create_test_deployment(id: &str, name: &str) -> Deployment {
        Deployment {
            id: DeploymentId::new(id),
            dataplane_id: DataPlaneId::new("cccccccc-cccc-cccc-cccc-cccccccccccc"),
            name: name.to_string(),
            kind: Some(DeploymentKind::Ferriskey),
            namespace: Some("aether-test".to_string()),
        }
    }

    /// A usage source no test asked anything of. The sync-cycle tests never
    /// reach it, and a mock with no expectation would panic if they did.
    fn unused_usage_source() -> Arc<MockUsageSource> {
        Arc::new(MockUsageSource::new())
    }

    /// A pod log source no test asked anything of.
    fn unused_pod_logs() -> Arc<MockPodLogSource> {
        Arc::new(MockPodLogSource::new())
    }

    fn create_test_action(deployment_id: &str, action_type: &str) -> Action {
        Action {
            id: ActionId(Uuid::new_v4()),
            deployment_id: DeploymentId::new(deployment_id),
            dataplane_id: DataPlaneId::new("cccccccc-cccc-cccc-cccc-cccccccccccc"),
            action_type: action_type.to_string(),
            payload: json!({"key": "value"}),
            version: 1,
            occurred_at: Utc::now(),
        }
    }

    struct HeraldServiceTestBuilder {
        control_plane: Arc<MockControlPlaneRepository>,
        message_bus: Arc<MockMessageBusRepository>,
        dataplane_id: DataPlaneId,
        shard_config: ShardConfig,
    }

    impl HeraldServiceTestBuilder {
        fn new() -> Self {
            Self {
                control_plane: Arc::new(MockControlPlaneRepository::new()),
                message_bus: Arc::new(MockMessageBusRepository::new()),
                dataplane_id: DataPlaneId::new("cccccccc-cccc-cccc-cccc-cccccccccccc"),
                // Owns every deployment by default so existing flow
                // assertions are unaffected by sharding.
                shard_config: ShardConfig::new(0, 1),
            }
        }

        fn with_control_plane(mut self, control_plane: MockControlPlaneRepository) -> Self {
            self.control_plane = Arc::new(control_plane);
            self
        }

        fn with_message_bus(mut self, message_bus: MockMessageBusRepository) -> Self {
            self.message_bus = Arc::new(message_bus);
            self
        }

        fn with_dataplane_id(mut self, dataplane_id: DataPlaneId) -> Self {
            self.dataplane_id = dataplane_id;
            self
        }

        fn build(
            self,
        ) -> HeraldServiceImpl<
            MockControlPlaneRepository,
            MockMessageBusRepository,
            MockOutcomeInboxRepository,
            MockUsageSource,
            MockPodLogSource,
        > {
            // Empty unless a test says otherwise: the outcome queue is not what
            // these tests are about, and a mock with no expectation set would
            // panic the moment the cycle drains it.
            let mut outcomes = MockOutcomeInboxRepository::new();
            outcomes
                .expect_drain()
                .returning(|_| Box::pin(async { Ok(Vec::new()) }));

            HeraldServiceImpl::new(
                self.control_plane,
                self.message_bus,
                Arc::new(outcomes),
                unused_usage_source(),
                unused_pod_logs(),
                self.dataplane_id,
                self.shard_config,
            )
        }
    }

    /// The gap this closes: Genesis removed the resources, said so on the bus,
    /// and nothing carried it to the control plane -- so the deployment stayed
    /// in `deleting` for ever.
    #[tokio::test]
    async fn a_sync_cycle_carries_queued_outcomes_to_the_control_plane() {
        let dataplane_id = DataPlaneId::new(Uuid::new_v4());
        let deployment_id = Uuid::new_v4();

        let mut outcomes = MockOutcomeInboxRepository::new();
        outcomes.expect_drain().times(1).returning(move |_| {
            Box::pin(async move {
                Ok(vec![DeploymentOutcomeReport {
                    version: None,
                    deployment_id,
                    outcome: "deleted".to_string(),
                }])
            })
        });

        let mut control_plane = MockControlPlaneRepository::new();
        control_plane
            .expect_send_heartbeat()
            .returning(|_| Box::pin(async { Ok(()) }));
        control_plane
            .expect_report_outcome()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        control_plane
            .expect_list_deployments()
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));

        let service = HeraldServiceImpl::new(
            Arc::new(control_plane),
            Arc::new(MockMessageBusRepository::new()),
            Arc::new(outcomes),
            unused_usage_source(),
            unused_pod_logs(),
            dataplane_id,
            ShardConfig::new(0, 1),
        );

        service
            .sync_all_deployments()
            .await
            .expect("cycle succeeds");
    }

    /// A control plane that cannot take a report must not stop deployments
    /// being claimed. The undelivered report leaves a status stale -- which is
    /// the situation that already existed -- while failing the cycle would stop
    /// work that has nothing to do with it.
    #[tokio::test]
    async fn a_cycle_survives_an_outcome_that_cannot_be_reported() {
        let dataplane_id = DataPlaneId::new(Uuid::new_v4());

        let mut outcomes = MockOutcomeInboxRepository::new();
        outcomes.expect_drain().returning(|_| {
            Box::pin(async {
                Ok(vec![DeploymentOutcomeReport {
                    version: None,
                    deployment_id: Uuid::new_v4(),
                    outcome: "deleted".to_string(),
                }])
            })
        });

        let mut control_plane = MockControlPlaneRepository::new();
        control_plane
            .expect_send_heartbeat()
            .returning(|_| Box::pin(async { Ok(()) }));
        control_plane.expect_report_outcome().returning(|_, _| {
            Box::pin(async {
                Err(HeraldError::ControlPlane {
                    message: "unavailable".to_string(),
                })
            })
        });
        control_plane
            .expect_list_deployments()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));

        let service = HeraldServiceImpl::new(
            Arc::new(control_plane),
            Arc::new(MockMessageBusRepository::new()),
            Arc::new(outcomes),
            unused_usage_source(),
            unused_pod_logs(),
            dataplane_id,
            ShardConfig::new(0, 1),
        );

        service
            .sync_all_deployments()
            .await
            .expect("the cycle continues past an undeliverable report");
    }

    /// An unreadable queue is the same class of problem, one layer earlier.
    #[tokio::test]
    async fn a_cycle_survives_an_unreadable_outcome_queue() {
        let mut outcomes = MockOutcomeInboxRepository::new();
        outcomes.expect_drain().returning(|_| {
            Box::pin(async {
                Err(HeraldError::MessageBus {
                    message: "broker gone".to_string(),
                })
            })
        });

        let mut control_plane = MockControlPlaneRepository::new();
        control_plane
            .expect_send_heartbeat()
            .returning(|_| Box::pin(async { Ok(()) }));
        control_plane
            .expect_list_deployments()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));

        let service = HeraldServiceImpl::new(
            Arc::new(control_plane),
            Arc::new(MockMessageBusRepository::new()),
            Arc::new(outcomes),
            unused_usage_source(),
            unused_pod_logs(),
            DataPlaneId::new(Uuid::new_v4()),
            ShardConfig::new(0, 1),
        );

        service
            .sync_all_deployments()
            .await
            .expect("cycle succeeds");
    }

    #[tokio::test]
    async fn test_sync_all_deployments_success() {
        // Arrange
        let deployment1 = Arc::new(create_test_deployment(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "deployment-one",
        ));
        let deployment2 = Arc::new(create_test_deployment(
            "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
            "deployment-two",
        ));

        let action1 = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.create",
        ));
        let action2 = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.update",
        ));
        let action3 = Arc::new(create_test_action(
            "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
            "postgres.create",
        ));

        let mut mock_control_plane = MockControlPlaneRepository::new();
        let d1 = deployment1.clone();
        let d2 = deployment2.clone();
        // Every sync cycle reports first; the tests that care about the
        // heartbeat itself assert on it explicitly.
        mock_control_plane
            .expect_send_heartbeat()
            .returning(|_| Box::pin(async { Ok(()) }));
        mock_control_plane
            .expect_list_deployments()
            .times(1)
            .returning(move |_| {
                let d1 = d1.clone();
                let d2 = d2.clone();
                Box::pin(async move { Ok(vec![(*d1).clone(), (*d2).clone()]) })
            });

        let a1 = action1.clone();
        let a2 = action2.clone();
        mock_control_plane
            .expect_claim_actions()
            .withf(|_, dep_id| dep_id.0 == "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")
            .times(1)
            .returning(move |_, _| {
                let a1 = a1.clone();
                let a2 = a2.clone();
                Box::pin(async move { Ok(vec![(*a1).clone(), (*a2).clone()]) })
            });

        let a3 = action3.clone();
        mock_control_plane
            .expect_claim_actions()
            .withf(|_, dep_id| dep_id.0 == "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb")
            .times(1)
            .returning(move |_, _| {
                let a3 = a3.clone();
                Box::pin(async move { Ok(vec![(*a3).clone()]) })
            });

        let mut mock_message_bus = MockMessageBusRepository::new();
        mock_message_bus
            .expect_publish()
            .times(3)
            .returning(|_| Box::pin(async { Ok(()) }));

        mock_control_plane
            .expect_ack_actions()
            .withf(|_, dep_id, published, failed| {
                dep_id.0 == "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"
                    && published.len() == 2
                    && failed.is_empty()
            })
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(AckOutcome { acknowledged: 2 }) }));

        mock_control_plane
            .expect_ack_actions()
            .withf(|_, dep_id, published, failed| {
                dep_id.0 == "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"
                    && published.len() == 1
                    && failed.is_empty()
            })
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(AckOutcome { acknowledged: 1 }) }));

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

        // Act
        let result = service.sync_all_deployments().await;

        // Assert: every action published, and each deployment's batch was
        // acked in a single call listing all of its actions as published.
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_sync_all_deployments_no_deployments() {
        // Arrange
        let mut mock_control_plane = MockControlPlaneRepository::new();
        // Every sync cycle reports first; the tests that care about the
        // heartbeat itself assert on it explicitly.
        mock_control_plane
            .expect_send_heartbeat()
            .returning(|_| Box::pin(async { Ok(()) }));
        mock_control_plane
            .expect_list_deployments()
            .times(1)
            .returning(|_| Box::pin(async { Ok(vec![]) }));

        let mock_message_bus = MockMessageBusRepository::new();

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

        // Act
        let result = service.sync_all_deployments().await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_sync_all_deployments_control_plane_error() {
        // Arrange
        let mut mock_control_plane = MockControlPlaneRepository::new();
        // Every sync cycle reports first; the tests that care about the
        // heartbeat itself assert on it explicitly.
        mock_control_plane
            .expect_send_heartbeat()
            .returning(|_| Box::pin(async { Ok(()) }));
        mock_control_plane
            .expect_list_deployments()
            .times(1)
            .returning(|_| {
                Box::pin(async {
                    Err(HeraldError::ControlPlane {
                        message: "Control plane error".to_string(),
                    })
                })
            });

        let mock_message_bus = MockMessageBusRepository::new();

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

        // Act
        let result = service.sync_all_deployments().await;

        // Assert
        assert!(result.is_err());
        if let Err(HeraldError::ControlPlane { message }) = result {
            assert_eq!(message, "Control plane error");
        } else {
            panic!("Expected ControlPlane error");
        }
    }

    #[tokio::test]
    async fn test_sync_all_deployments_partial_publish_failure_acks_one_batch() {
        // Arrange: two actions for the same deployment, one publishes fine,
        // the other fails. A publish failure must not abort the batch nor
        // the sync: both outcomes are reported in a single ack call.
        let deployment = Arc::new(create_test_deployment(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "deployment-one",
        ));
        let ok_action = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.create",
        ));
        let failing_action = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.update",
        ));
        let ok_id = ok_action.id;
        let failing_id = failing_action.id;

        let mut mock_control_plane = MockControlPlaneRepository::new();
        let d = deployment.clone();
        // Every sync cycle reports first; the tests that care about the
        // heartbeat itself assert on it explicitly.
        mock_control_plane
            .expect_send_heartbeat()
            .returning(|_| Box::pin(async { Ok(()) }));
        mock_control_plane
            .expect_list_deployments()
            .times(1)
            .returning(move |_| {
                let d = d.clone();
                Box::pin(async move { Ok(vec![(*d).clone()]) })
            });

        let a1 = ok_action.clone();
        let a2 = failing_action.clone();
        mock_control_plane
            .expect_claim_actions()
            .times(1)
            .returning(move |_, _| {
                let a1 = a1.clone();
                let a2 = a2.clone();
                Box::pin(async move { Ok(vec![(*a1).clone(), (*a2).clone()]) })
            });

        let mut mock_message_bus = MockMessageBusRepository::new();
        mock_message_bus
            .expect_publish()
            .withf(move |event| event.action_id == ok_id.0)
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        mock_message_bus
            .expect_publish()
            .withf(move |event| event.action_id == failing_id.0)
            .times(1)
            .returning(|_| {
                Box::pin(async {
                    Err(HeraldError::MessageBus {
                        message: "Message bus error".to_string(),
                    })
                })
            });

        mock_control_plane
            .expect_ack_actions()
            .withf(move |_, _, published, failed| {
                published == &vec![ok_id]
                    && failed
                        == &vec![AckFailure {
                            action_id: failing_id,
                            reason: ActionFailureReason::PublishFailed,
                        }]
            })
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(AckOutcome { acknowledged: 1 }) }));

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

        // Act
        let result = service.sync_all_deployments().await;

        // Assert: the publish failure was recorded via the ack, not
        // propagated, so the sync itself still succeeds.
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_sync_all_deployments_ack_failure_does_not_fail_sync() {
        // Arrange: publish succeeds but the control plane's ack call fails.
        // The publish must not be lost or retried, and the sync must not
        // fail — the ack is best-effort, the control plane's ack is
        // idempotent, and a lost ack just means the lease expires later.
        let deployment = Arc::new(create_test_deployment(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "deployment-one",
        ));
        let action = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.create",
        ));

        let mut mock_control_plane = MockControlPlaneRepository::new();
        let d = deployment.clone();
        // Every sync cycle reports first; the tests that care about the
        // heartbeat itself assert on it explicitly.
        mock_control_plane
            .expect_send_heartbeat()
            .returning(|_| Box::pin(async { Ok(()) }));
        mock_control_plane
            .expect_list_deployments()
            .times(1)
            .returning(move |_| {
                let d = d.clone();
                Box::pin(async move { Ok(vec![(*d).clone()]) })
            });

        let a = action.clone();
        mock_control_plane
            .expect_claim_actions()
            .times(1)
            .returning(move |_, _| {
                let a = a.clone();
                Box::pin(async move { Ok(vec![(*a).clone()]) })
            });

        mock_control_plane
            .expect_ack_actions()
            .times(1)
            .returning(|_, _, _, _| {
                Box::pin(async {
                    Err(HeraldError::ControlPlane {
                        message: "ack endpoint unreachable".to_string(),
                    })
                })
            });

        let mut mock_message_bus = MockMessageBusRepository::new();
        mock_message_bus
            .expect_publish()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

        // Act
        let result = service.sync_all_deployments().await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_deployment_success() {
        // Arrange
        let deployment_id = DeploymentId::new("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let action1 = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.create",
        ));
        let action2 = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.update",
        ));

        let mut mock_control_plane = MockControlPlaneRepository::new();
        let a1 = action1.clone();
        let a2 = action2.clone();
        mock_control_plane
            .expect_claim_actions()
            .times(1)
            .returning(move |_, _| {
                let a1 = a1.clone();
                let a2 = a2.clone();
                Box::pin(async move { Ok(vec![(*a1).clone(), (*a2).clone()]) })
            });

        let mut mock_message_bus = MockMessageBusRepository::new();
        mock_message_bus
            .expect_publish()
            .times(2)
            .returning(|_| Box::pin(async { Ok(()) }));

        mock_control_plane
            .expect_ack_actions()
            .withf(|_, _, published, failed| published.len() == 2 && failed.is_empty())
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(AckOutcome { acknowledged: 2 }) }));

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

        // Act
        let result = service.process_deployment(&deployment_id).await;

        // Assert: both actions published, then acked together in one call.
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_deployment_no_actions() {
        // Arrange
        let deployment_id = DeploymentId::new("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");

        let mut mock_control_plane = MockControlPlaneRepository::new();
        mock_control_plane
            .expect_claim_actions()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(vec![]) }));

        let mock_message_bus = MockMessageBusRepository::new();

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

        // Act
        let result = service.process_deployment(&deployment_id).await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_deployment_claim_actions_error() {
        // Arrange
        let deployment_id = DeploymentId::new("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");

        let mut mock_control_plane = MockControlPlaneRepository::new();
        mock_control_plane
            .expect_claim_actions()
            .times(1)
            .returning(|_, _| {
                Box::pin(async {
                    Err(HeraldError::ControlPlane {
                        message: "Cannot claim actions".to_string(),
                    })
                })
            });

        let mock_message_bus = MockMessageBusRepository::new();

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

        // Act
        let result = service.process_deployment(&deployment_id).await;

        // Assert
        assert!(result.is_err());
        if let Err(HeraldError::ControlPlane { message }) = result {
            assert_eq!(message, "Cannot claim actions");
        } else {
            panic!("Expected ControlPlane error");
        }
    }

    #[tokio::test]
    async fn test_process_deployment_partial_publish_failure_acks_one_batch() {
        // Arrange: one action publishes fine, the other fails. Both
        // outcomes must be reported in a single ack call, and the failure
        // must not abort processing of the successful one.
        let deployment_id = DeploymentId::new("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let ok_action = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.create",
        ));
        let failing_action = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.update",
        ));
        let ok_id = ok_action.id;
        let failing_id = failing_action.id;

        let mut mock_control_plane = MockControlPlaneRepository::new();
        let a1 = ok_action.clone();
        let a2 = failing_action.clone();
        mock_control_plane
            .expect_claim_actions()
            .times(1)
            .returning(move |_, _| {
                let a1 = a1.clone();
                let a2 = a2.clone();
                Box::pin(async move { Ok(vec![(*a1).clone(), (*a2).clone()]) })
            });

        let mut mock_message_bus = MockMessageBusRepository::new();
        mock_message_bus
            .expect_publish()
            .withf(move |event| event.action_id == ok_id.0)
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        mock_message_bus
            .expect_publish()
            .withf(move |event| event.action_id == failing_id.0)
            .times(1)
            .returning(|_| {
                Box::pin(async {
                    Err(HeraldError::MessageBus {
                        message: "Publish failed".to_string(),
                    })
                })
            });

        mock_control_plane
            .expect_ack_actions()
            .withf(move |_, _, published, failed| {
                published == &vec![ok_id]
                    && failed
                        == &vec![AckFailure {
                            action_id: failing_id,
                            reason: ActionFailureReason::PublishFailed,
                        }]
            })
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(AckOutcome { acknowledged: 1 }) }));

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

        // Act
        let result = service.process_deployment(&deployment_id).await;

        // Assert: the publish failure was recorded via the ack, not
        // propagated.
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_deployment_ack_failure_does_not_fail_processing() {
        // Arrange: publish succeeds but acking fails. The failure must be
        // swallowed (logged, not propagated) rather than reported to the
        // caller or panicking.
        let deployment_id = DeploymentId::new("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa");
        let action = Arc::new(create_test_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "ferriskey.create",
        ));

        let mut mock_control_plane = MockControlPlaneRepository::new();
        let a = action.clone();
        mock_control_plane
            .expect_claim_actions()
            .times(1)
            .returning(move |_, _| {
                let a = a.clone();
                Box::pin(async move { Ok(vec![(*a).clone()]) })
            });

        mock_control_plane
            .expect_ack_actions()
            .times(1)
            .returning(|_, _, _, _| {
                Box::pin(async {
                    Err(HeraldError::ControlPlane {
                        message: "ack endpoint unreachable".to_string(),
                    })
                })
            });

        let mut mock_message_bus = MockMessageBusRepository::new();
        mock_message_bus
            .expect_publish()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

        // Act
        let result = service.process_deployment(&deployment_id).await;

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_herald_service_impl_creation() {
        let _service = HeraldServiceTestBuilder::new()
            .with_dataplane_id(DataPlaneId::new("test-dp-123"))
            .build();
    }

    /// The heartbeat is the whole point of the sync cycle for a data plane
    /// holding no deployments: without it, an idle cluster and a dead one look
    /// identical to the control plane.
    #[tokio::test]
    async fn every_sync_cycle_reports_the_heartbeat_even_with_no_deployments() {
        let mut mock_control_plane = MockControlPlaneRepository::new();
        mock_control_plane
            .expect_send_heartbeat()
            .times(1)
            .withf(|dp_id| dp_id.0 == "cccccccc-cccc-cccc-cccc-cccccccccccc")
            .returning(|_| Box::pin(async { Ok(()) }));
        mock_control_plane
            .expect_list_deployments()
            .times(1)
            .returning(|_| Box::pin(async { Ok(vec![]) }));

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(MockMessageBusRepository::new())
            .build();

        assert!(service.sync_all_deployments().await.is_ok());
    }

    /// A failed heartbeat costs at most one missed window. Letting it abort the
    /// cycle would turn a transient control-plane hiccup into a data plane that
    /// stops doing the work it could still do.
    #[tokio::test]
    async fn a_failing_heartbeat_does_not_stop_the_cycle() {
        let deployment =
            create_test_deployment("11111111-1111-1111-1111-111111111111", "deployment-one");
        let action =
            create_test_action("11111111-1111-1111-1111-111111111111", "deployment.create");

        let mut mock_control_plane = MockControlPlaneRepository::new();
        mock_control_plane.expect_send_heartbeat().returning(|_| {
            Box::pin(async {
                Err(HeraldError::ControlPlane {
                    message: "heartbeat unavailable".to_string(),
                })
            })
        });
        let d = deployment.clone();
        mock_control_plane
            .expect_list_deployments()
            .returning(move |_| {
                let d = d.clone();
                Box::pin(async move { Ok(vec![d.clone()]) })
            });
        let a = action.clone();
        mock_control_plane
            .expect_claim_actions()
            .returning(move |_, _| {
                let a = a.clone();
                Box::pin(async move { Ok(vec![a.clone()]) })
            });
        mock_control_plane
            .expect_ack_actions()
            .times(1)
            .returning(|_, _, _, _| Box::pin(async { Ok(AckOutcome { acknowledged: 1 }) }));

        let mut mock_message_bus = MockMessageBusRepository::new();
        mock_message_bus
            .expect_publish()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));

        let service = HeraldServiceTestBuilder::new()
            .with_control_plane(mock_control_plane)
            .with_message_bus(mock_message_bus)
            .build();

        assert!(
            service.sync_all_deployments().await.is_ok(),
            "the actions were published and acknowledged; the heartbeat is not the work"
        );
    }

    // --- usage collection ---------------------------------------------------

    use crate::domain::entities::usage::{UsageBucket, UsagePoint};
    use std::collections::VecDeque;
    use std::sync::Mutex as StdMutex;

    /// A source that answers with a fixed script, one entry per call.
    fn scripted_source(
        script: Vec<Result<Option<CounterSample>, HeraldError>>,
    ) -> Arc<MockUsageSource> {
        let script = Arc::new(StdMutex::new(VecDeque::from(script)));
        let mut source = MockUsageSource::new();
        source.expect_sample().returning(move |_| {
            let next = script
                .lock()
                .expect("the script")
                .pop_front()
                .expect("the script has a reading for every call");
            Box::pin(async move { next })
        });

        Arc::new(source)
    }

    /// Two readings either side of a minute boundary.
    ///
    /// The first only establishes the baseline -- and lands in a minute Herald
    /// joined halfway through, which is never reported -- so the second is the
    /// first reading that can be attributed to a whole bucket.
    fn straddling(first: u64, second: u64) -> (Vec<CounterSample>, DateTime<Utc>) {
        let boundary = UsageBucket::containing(Utc::now() - Duration::minutes(2)).start();

        (
            vec![
                CounterSample::new(boundary - Duration::seconds(5))
                    .with(UsageMetric::Requests, first),
                CounterSample::new(boundary + Duration::seconds(10))
                    .with(UsageMetric::Requests, second),
            ],
            boundary,
        )
    }

    type ReportedUsage = Arc<StdMutex<Vec<(DeploymentId, Vec<UsagePoint>)>>>;

    /// A control plane that lists one deployment and records everything
    /// reported against it.
    fn listing_control_plane(
        deployment: Deployment,
    ) -> (MockControlPlaneRepository, ReportedUsage) {
        let reported: ReportedUsage = Arc::new(StdMutex::new(Vec::new()));

        let mut control_plane = MockControlPlaneRepository::new();
        control_plane.expect_list_deployments().returning(move |_| {
            let deployment = deployment.clone();
            Box::pin(async move { Ok(vec![deployment]) })
        });

        let recorder = Arc::clone(&reported);
        control_plane
            .expect_report_usage()
            .returning(move |deployment_id, points| {
                recorder
                    .lock()
                    .expect("the recorder")
                    .push((deployment_id.clone(), points.to_vec()));
                Box::pin(async { Ok(()) })
            });

        (control_plane, reported)
    }

    fn usage_service(
        control_plane: MockControlPlaneRepository,
        source: Arc<MockUsageSource>,
    ) -> HeraldServiceImpl<
        MockControlPlaneRepository,
        MockMessageBusRepository,
        MockOutcomeInboxRepository,
        MockUsageSource,
        MockPodLogSource,
    > {
        HeraldServiceImpl::new(
            Arc::new(control_plane),
            Arc::new(MockMessageBusRepository::new()),
            Arc::new(MockOutcomeInboxRepository::new()),
            source,
            unused_pod_logs(),
            DataPlaneId::new("cccccccc-cccc-cccc-cccc-cccccccccccc"),
            ShardConfig::new(0, 1),
        )
    }

    fn reported_points(reported: &ReportedUsage) -> Vec<UsagePoint> {
        reported
            .lock()
            .expect("the recorder")
            .iter()
            .flat_map(|(_, points)| points.clone())
            .collect()
    }

    /// The acceptance criterion, at the level where it is actually decided: an
    /// instance that could not be read contributes no bucket at all, so the
    /// control plane is never told anything about that minute.
    #[tokio::test]
    async fn an_unreachable_instance_reports_no_bucket_rather_than_a_zero() {
        let deployment = create_test_deployment("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "acme");
        let (control_plane, reported) = listing_control_plane(deployment);

        let source = scripted_source(vec![
            Err(HeraldError::UsageSource {
                message: "connection refused".to_string(),
            }),
            Err(HeraldError::UsageSource {
                message: "connection refused".to_string(),
            }),
        ]);

        let service = usage_service(control_plane, source);

        service.collect_usage().await.expect("the cycle succeeds");
        service.collect_usage().await.expect("the cycle succeeds");

        assert!(
            reported.lock().expect("the recorder").is_empty(),
            "an instance that went quiet must leave a hole, not a zero: {:?}",
            reported_points(&reported)
        );
    }

    /// And the other half: an instance that answered and served nothing does
    /// report a zero, because that is a measurement.
    #[tokio::test]
    async fn a_reachable_instance_with_no_traffic_reports_a_zero() {
        let deployment = create_test_deployment("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "acme");
        let (control_plane, reported) = listing_control_plane(deployment);

        let (readings, boundary) = straddling(500, 500);
        let source = scripted_source(readings.into_iter().map(|s| Ok(Some(s))).collect());

        let service = usage_service(control_plane, source);
        service.collect_usage().await.expect("the cycle succeeds");
        service.collect_usage().await.expect("the cycle succeeds");

        let points = reported_points(&reported);
        assert_eq!(
            points,
            vec![UsagePoint {
                metric: UsageMetric::Requests,
                bucket: UsageBucket::containing(boundary),
                value: 0,
            }],
            "got {points:?}"
        );
    }

    /// A source that has nothing to offer is not a failure and not a zero: the
    /// deployment simply never appears in the usage the control plane holds.
    #[tokio::test]
    async fn a_product_that_exposes_nothing_reports_nothing() {
        let deployment = create_test_deployment("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "acme");
        let (control_plane, reported) = listing_control_plane(deployment);

        let source = scripted_source(vec![Ok(None), Ok(None)]);
        let service = usage_service(control_plane, source);

        service.collect_usage().await.expect("the cycle succeeds");
        service.collect_usage().await.expect("the cycle succeeds");

        assert!(reported.lock().expect("the recorder").is_empty());
    }

    /// A deployment the control plane described without a namespace has
    /// nowhere to be read, and that must not stop the cycle.
    #[tokio::test]
    async fn a_deployment_with_nowhere_to_be_read_is_skipped() {
        let mut deployment = create_test_deployment("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "acme");
        deployment.namespace = None;

        let (control_plane, reported) = listing_control_plane(deployment);

        let mut source = MockUsageSource::new();
        source.expect_sample().never();

        let service = usage_service(control_plane, Arc::new(source));
        service.collect_usage().await.expect("the cycle succeeds");

        assert!(reported.lock().expect("the recorder").is_empty());
    }

    /// A report that did not land is not lost: the bucket stays in the window
    /// and is offered again, unchanged, on the next cycle. This is the half of
    /// "no gap" that survives a control plane blip.
    #[tokio::test]
    async fn a_bucket_whose_report_failed_is_sent_again_next_cycle() {
        let deployment = create_test_deployment("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "acme");
        let boundary = UsageBucket::containing(Utc::now() - Duration::minutes(2)).start();

        let mut control_plane = MockControlPlaneRepository::new();
        let listed = deployment.clone();
        control_plane.expect_list_deployments().returning(move |_| {
            let listed = listed.clone();
            Box::pin(async move { Ok(vec![listed]) })
        });

        let attempts: ReportedUsage = Arc::new(StdMutex::new(Vec::new()));
        let recorder = Arc::clone(&attempts);
        control_plane
            .expect_report_usage()
            .returning(move |deployment_id, points| {
                let mut attempts = recorder.lock().expect("the recorder");
                attempts.push((deployment_id.clone(), points.to_vec()));
                let first = attempts.len() == 1;
                Box::pin(async move {
                    if first {
                        Err(HeraldError::ControlPlane {
                            message: "unavailable".to_string(),
                        })
                    } else {
                        Ok(())
                    }
                })
            });

        let source = scripted_source(vec![
            Ok(Some(
                CounterSample::new(boundary - Duration::seconds(5))
                    .with(UsageMetric::Requests, 500),
            )),
            Ok(Some(
                CounterSample::new(boundary + Duration::seconds(10))
                    .with(UsageMetric::Requests, 530),
            )),
            Ok(Some(
                CounterSample::new(boundary + Duration::seconds(25))
                    .with(UsageMetric::Requests, 530),
            )),
        ]);

        let service = usage_service(control_plane, source);
        service.collect_usage().await.expect("the cycle succeeds");
        service.collect_usage().await.expect("the cycle succeeds");
        service.collect_usage().await.expect("the cycle succeeds");

        let attempts = attempts.lock().expect("the recorder");
        assert_eq!(attempts.len(), 2, "the failed report must be retried");
        assert_eq!(
            attempts[0].1, attempts[1].1,
            "the retry must carry the same bucket and the same value"
        );
    }

    /// A restart, through the service. The replacement process must not credit
    /// the traffic that accumulated while nothing was watching to the minute it
    /// happened to come up in -- that minute would then report usage that did
    /// not occur in it, and would overwrite whatever the dead process wrote.
    #[tokio::test]
    async fn a_restarted_herald_neither_gaps_nor_duplicates_the_minute_it_returns_in() {
        let deployment = create_test_deployment("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "acme");
        let boundary = UsageBucket::containing(Utc::now() - Duration::minutes(3)).start();

        let (before_control_plane, before_reported) = listing_control_plane(deployment.clone());
        let before = usage_service(
            before_control_plane,
            scripted_source(vec![
                Ok(Some(
                    CounterSample::new(boundary - Duration::seconds(5))
                        .with(UsageMetric::Requests, 500),
                )),
                Ok(Some(
                    CounterSample::new(boundary + Duration::seconds(10))
                        .with(UsageMetric::Requests, 530),
                )),
            ]),
        );
        before.collect_usage().await.expect("the cycle succeeds");
        before.collect_usage().await.expect("the cycle succeeds");

        let before_points = reported_points(&before_reported);
        assert_eq!(before_points.len(), 1);
        assert_eq!(before_points[0].value, 30);

        // The process dies, and the instance serves another 400 requests
        // before the replacement comes up in the following minute.
        let (after_control_plane, after_reported) = listing_control_plane(deployment);
        let after = usage_service(
            after_control_plane,
            scripted_source(vec![
                Ok(Some(
                    CounterSample::new(boundary + Duration::seconds(70))
                        .with(UsageMetric::Requests, 930),
                )),
                Ok(Some(
                    CounterSample::new(boundary + Duration::seconds(85))
                        .with(UsageMetric::Requests, 935),
                )),
                Ok(Some(
                    CounterSample::new(boundary + Duration::seconds(130))
                        .with(UsageMetric::Requests, 940),
                )),
            ]),
        );
        after.collect_usage().await.expect("the cycle succeeds");
        after.collect_usage().await.expect("the cycle succeeds");
        after.collect_usage().await.expect("the cycle succeeds");

        let after_points = reported_points(&after_reported);

        assert!(
            after_points
                .iter()
                .all(|point| point.bucket != before_points[0].bucket),
            "the minute already reported must not be rewritten from a partial view: {after_points:?}"
        );
        assert!(
            after_points
                .iter()
                .all(|point| point.bucket.start() != boundary + Duration::seconds(60)),
            "the minute the restart landed in was only partly watched: {after_points:?}"
        );
        assert_eq!(
            after_points
                .iter()
                .find(|point| point.bucket.start() == boundary + Duration::seconds(120))
                .map(|point| point.value),
            Some(5),
            "the first whole minute after the restart reports only its own traffic: {after_points:?}"
        );
    }

    // --- log sessions -------------------------------------------------------

    use crate::domain::entities::logs::{LOG_ACTION_TYPE, LogLine};
    use crate::domain::ports::LogPushOutcome;
    use tokio::sync::mpsc;

    fn log_action(deployment_id: &str, session_id: &str) -> Action {
        Action {
            id: ActionId(Uuid::new_v4()),
            deployment_id: DeploymentId::new(deployment_id),
            dataplane_id: DataPlaneId::new("cccccccc-cccc-cccc-cccc-cccccccccccc"),
            action_type: LOG_ACTION_TYPE.to_string(),
            payload: json!({
                "deployment_id": deployment_id,
                "dataplane_id": "cccccccc-cccc-cccc-cccc-cccccccccccc",
                "namespace": "aether-acme",
                "kind": "ferriskey",
                "session_id": session_id,
                "since_minutes": 10
            }),
            version: 1,
            occurred_at: Utc::now(),
        }
    }

    type AckedActions = Arc<StdMutex<Vec<(Vec<ActionId>, Vec<AckFailure>)>>>;

    /// A control plane that hands out one action for one deployment and
    /// records how it was acknowledged.
    fn control_plane_serving(action: Action) -> (MockControlPlaneRepository, AckedActions) {
        let deployment = create_test_deployment(&action.deployment_id.0, "acme");
        let acked: AckedActions = Arc::new(StdMutex::new(Vec::new()));

        let mut control_plane = MockControlPlaneRepository::new();
        control_plane
            .expect_send_heartbeat()
            .returning(|_| Box::pin(async { Ok(()) }));
        control_plane.expect_list_deployments().returning(move |_| {
            let deployment = deployment.clone();
            Box::pin(async move { Ok(vec![deployment]) })
        });
        control_plane.expect_claim_actions().returning(move |_, _| {
            let action = action.clone();
            Box::pin(async move { Ok(vec![action]) })
        });

        let recorder = Arc::clone(&acked);
        control_plane
            .expect_ack_actions()
            .returning(move |_, _, published, failed| {
                recorder
                    .lock()
                    .expect("the recorder")
                    .push((published, failed));
                Box::pin(async { Ok(AckOutcome { acknowledged: 1 }) })
            });
        control_plane
            .expect_push_log_lines()
            .returning(|_, _, _| Box::pin(async { Ok(LogPushOutcome::Relayed) }));

        (control_plane, acked)
    }

    /// A source that reports each session it was asked to follow and then
    /// holds it open, so a session started in one cycle is still running in
    /// the next.
    fn source_that_holds_sessions_open() -> (MockPodLogSource, mpsc::Receiver<()>) {
        let (started, receiver) = mpsc::channel(8);
        let senders: Arc<StdMutex<Vec<mpsc::Sender<LogLine>>>> =
            Arc::new(StdMutex::new(Vec::new()));

        let mut source = MockPodLogSource::new();
        source.expect_follow().returning(move |_| {
            let started = started.clone();
            let senders = Arc::clone(&senders);
            Box::pin(async move {
                let (sender, lines) = mpsc::channel(8);
                // Held rather than dropped: a live sender is a session that
                // has not ended.
                senders.lock().expect("the senders").push(sender);
                started.send(()).await.ok();
                Ok(lines)
            })
        });

        (source, receiver)
    }

    fn log_service(
        control_plane: MockControlPlaneRepository,
        message_bus: MockMessageBusRepository,
        pod_logs: MockPodLogSource,
    ) -> HeraldServiceImpl<
        MockControlPlaneRepository,
        MockMessageBusRepository,
        MockOutcomeInboxRepository,
        MockUsageSource,
        MockPodLogSource,
    > {
        let mut outcomes = MockOutcomeInboxRepository::new();
        outcomes
            .expect_drain()
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));

        HeraldServiceImpl::new(
            Arc::new(control_plane),
            Arc::new(message_bus),
            Arc::new(outcomes),
            unused_usage_source(),
            Arc::new(pod_logs),
            DataPlaneId::new("cccccccc-cccc-cccc-cccc-cccccccccccc"),
            ShardConfig::new(0, 1),
        )
    }

    /// Every other action is work for another component and goes to the bus.
    /// This one is work for Herald, and Genesis has no handler for it -- a
    /// published log request would sit in a queue until it was dead-lettered.
    #[tokio::test]
    async fn a_log_request_is_handled_here_rather_than_published_to_the_bus() {
        let action = log_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "33333333-3333-3333-3333-333333333333",
        );
        let action_id = action.id;

        let (control_plane, acked) = control_plane_serving(action);
        let (pod_logs, mut started) = source_that_holds_sessions_open();

        let mut message_bus = MockMessageBusRepository::new();
        message_bus.expect_publish().never();

        let service = log_service(control_plane, message_bus, pod_logs);
        service
            .sync_all_deployments()
            .await
            .expect("cycle succeeds");

        started.recv().await.expect("the session started");

        let acked = acked.lock().expect("the recorder");
        assert_eq!(acked.len(), 1);
        assert_eq!(acked[0].0, vec![action_id], "the action was taken");
        assert!(acked[0].1.is_empty(), "and not failed");
    }

    /// Delivery is at-least-once, so the same request arrives again whenever
    /// an ack is lost. A second stream would double every line on the screen.
    #[tokio::test]
    async fn a_log_request_delivered_twice_follows_the_pods_once() {
        let action = log_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "33333333-3333-3333-3333-333333333333",
        );

        let (control_plane, _) = control_plane_serving(action);
        let (pod_logs, mut started) = source_that_holds_sessions_open();

        let mut message_bus = MockMessageBusRepository::new();
        message_bus.expect_publish().never();

        let service = log_service(control_plane, message_bus, pod_logs);

        service
            .sync_all_deployments()
            .await
            .expect("cycle succeeds");
        started.recv().await.expect("the session started");

        service
            .sync_all_deployments()
            .await
            .expect("cycle succeeds");

        assert!(
            started.try_recv().is_err(),
            "the same session must not be followed a second time"
        );
    }

    /// A request Herald cannot make sense of is reported as failed rather
    /// than left to expire and be redelivered for ever.
    #[tokio::test]
    async fn an_unusable_log_request_is_acknowledged_as_failed() {
        let mut action = log_action(
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "33333333-3333-3333-3333-333333333333",
        );
        action.payload["since_minutes"] = json!(1_440);
        let action_id = action.id;

        let (control_plane, acked) = control_plane_serving(action);

        let mut pod_logs = MockPodLogSource::new();
        pod_logs.expect_follow().never();

        let mut message_bus = MockMessageBusRepository::new();
        message_bus.expect_publish().never();

        let service = log_service(control_plane, message_bus, pod_logs);
        service
            .sync_all_deployments()
            .await
            .expect("cycle succeeds");

        let acked = acked.lock().expect("the recorder");
        assert!(acked[0].0.is_empty(), "nothing was taken on");
        assert_eq!(acked[0].1.len(), 1);
        assert_eq!(acked[0].1[0].action_id, action_id);
        assert_eq!(
            acked[0].1[0].reason,
            ActionFailureReason::InvalidPayload,
            "a window beyond the cap is a malformed request"
        );
    }
}
