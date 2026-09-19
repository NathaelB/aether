//! Turning a `deployment.drill` action into a restore of the deployment's
//! own archive, into a throwaway namespace, verified and torn down (#185).
//!
//! The point of the whole feature is here: a restore time objective measured
//! rather than claimed, and a corrupted archive that fails the drill rather
//! than passing quietly.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tracing::{info, warn};

use crate::domain::entities::action_event::ActionEvent;
use crate::domain::entities::drill_payload::DrillPayloadV1;
use crate::domain::entities::identity_instance::{
    DesiredDatabase, DesiredIdentityInstance, DesiredRestore, IdentityInstanceProvider,
    IdentityInstanceRef,
};
use crate::domain::entities::outcome::DeploymentOutcomeReport;
use crate::domain::error::GenesisError;
use crate::domain::ports::{
    BoxFuture, DatabaseProbe, EventHandler, IdentityInstancePort, OutcomePublisher,
};

/// How long a drill waits for its throwaway instance to come up before
/// giving up and reporting that it failed. Bounded on purpose: the whole
/// point is a real answer, not a run that waits forever for one, and the
/// capacity it holds is real the same way an upgrade's window is.
const READY_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Whether the drill got far enough to report an outcome, or failed in a way
/// that means the message itself must be retried.
enum DrillFailure {
    /// The drill ran and did not prove the restore -- an outcome to report,
    /// not a processing error. A corrupted archive lands here.
    Verification(String),
    /// The machinery itself failed before there was anything to verify --
    /// retried like any other handler error, never reported as a drill
    /// result.
    Infrastructure(GenesisError),
}

pub struct DrillEventHandler {
    identity_instances: Arc<dyn IdentityInstancePort>,
    database: Arc<dyn DatabaseProbe>,
    outcomes: Arc<dyn OutcomePublisher>,
    ready_timeout: Duration,
    poll_interval: Duration,
}

impl DrillEventHandler {
    pub fn new(
        identity_instances: Arc<dyn IdentityInstancePort>,
        database: Arc<dyn DatabaseProbe>,
        outcomes: Arc<dyn OutcomePublisher>,
    ) -> Self {
        Self {
            identity_instances,
            database,
            outcomes,
            ready_timeout: READY_TIMEOUT,
            poll_interval: POLL_INTERVAL,
        }
    }

    /// Only for tests: a run that waits minutes for readiness has no place
    /// in a unit test suite, so the window this drives is overridable rather
    /// than only ever the constant above.
    #[cfg(test)]
    fn with_timing(mut self, ready_timeout: Duration, poll_interval: Duration) -> Self {
        self.ready_timeout = ready_timeout;
        self.poll_interval = poll_interval;
        self
    }

    /// Named after the action, never after the deployment.
    ///
    /// The global status watcher recovers a deployment id from exactly the
    /// `deployment-` prefix. A throwaway instance named like a real one would
    /// have its readiness reported as if it were the deployment being
    /// drilled -- naming it after the action instead keeps the watcher from
    /// ever seeing it as one.
    fn reference(action_id: uuid::Uuid) -> IdentityInstanceRef {
        IdentityInstanceRef {
            name: format!("drill-{action_id}"),
            namespace: format!("drill-{action_id}"),
        }
    }

    fn desired(
        payload: &DrillPayloadV1,
        reference: &IdentityInstanceRef,
    ) -> Result<DesiredIdentityInstance, GenesisError> {
        let provider = IdentityInstanceProvider::parse(&payload.kind)?;

        Ok(DesiredIdentityInstance {
            reference: reference.clone(),
            organisation_id: payload.organisation_id.to_string(),
            provider,
            version: payload.version.clone(),
            // Never resolved and never fronted by an ingress: the drill
            // talks to this instance's database directly, and a hostname is
            // here only because the CRD requires one.
            hostname: format!("{}.drill.internal", reference.name),
            database: DesiredDatabase::from_reserved(
                payload.cpu_millis,
                payload.memory_mib,
                payload.storage_gib,
            ),
            archive: None,
            restore: Some(DesiredRestore {
                destination_path: payload.source.destination_path.clone(),
                server_name: payload.source.server_name.clone(),
                backup_id: payload.source.backup_id.clone(),
            }),
        })
    }

    async fn wait_until_ready(&self, reference: &IdentityInstanceRef) -> Result<(), DrillFailure> {
        let deadline = Instant::now() + self.ready_timeout;

        loop {
            let ready = self
                .identity_instances
                .is_ready(reference)
                .await
                .map_err(DrillFailure::Infrastructure)?;

            if ready {
                return Ok(());
            }

            if Instant::now() >= deadline {
                return Err(DrillFailure::Verification(format!(
                    "the restored instance did not become ready within {:?} -- \
                     the restore itself may have failed",
                    self.ready_timeout
                )));
            }

            tokio::time::sleep(self.poll_interval).await;
        }
    }

    async fn run(
        &self,
        payload: &DrillPayloadV1,
        reference: &IdentityInstanceRef,
    ) -> Result<(), DrillFailure> {
        let desired = Self::desired(payload, reference).map_err(DrillFailure::Infrastructure)?;

        self.identity_instances
            .apply(&desired)
            .await
            .map_err(DrillFailure::Infrastructure)?;

        self.wait_until_ready(reference).await?;

        let uri = self
            .identity_instances
            .database_uri(reference)
            .await
            .map_err(|error| {
                DrillFailure::Verification(format!(
                    "the restored instance reported ready but its connection secret was \
                     not usable: {error}"
                ))
            })?;

        self.database
            .answers_a_query(&uri)
            .await
            .map_err(|error| DrillFailure::Verification(error.to_string()))
    }
}

impl EventHandler for DrillEventHandler {
    fn routing_key(&self) -> &str {
        "deployment.drill"
    }

    fn handle<'a>(&'a self, event: ActionEvent) -> BoxFuture<'a, Result<(), GenesisError>> {
        Box::pin(async move {
            let payload = DrillPayloadV1::from_value(&event.payload)?;
            let reference = Self::reference(event.action_id);

            info!(
                deployment_id = %payload.deployment_id,
                namespace = %reference.namespace,
                "starting a restore drill"
            );

            let started = Instant::now();
            let outcome = self.run(&payload, &reference).await;

            // Torn down regardless of how the drill went: what it created
            // belongs to nobody once the answer is in, and the capacity a
            // namespace left behind after a failure would keep consuming is
            // exactly what the issue warns against.
            if let Err(error) = self
                .identity_instances
                .delete_namespace(&reference.namespace)
                .await
            {
                warn!(
                    namespace = %reference.namespace,
                    %error,
                    "failed to remove the drill namespace; retrying the whole drill"
                );
                return Err(error);
            }

            let elapsed = started.elapsed().as_secs();

            let report = match outcome {
                Ok(()) => DeploymentOutcomeReport::drill_succeeded(payload.deployment_id, elapsed),
                Err(DrillFailure::Verification(reason)) => {
                    warn!(
                        deployment_id = %payload.deployment_id,
                        reason = %reason,
                        "restore drill failed"
                    );
                    DeploymentOutcomeReport::drill_failed(
                        payload.deployment_id,
                        reason,
                        Some(elapsed),
                    )
                }
                Err(DrillFailure::Infrastructure(error)) => return Err(error),
            };

            self.outcomes.publish(report).await
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    const DEPLOYMENT: Uuid = Uuid::from_u128(1);
    const ORGANISATION: Uuid = Uuid::from_u128(2);
    const ACTION: Uuid = Uuid::from_u128(7);

    /// Ready after `ready_after` calls to `is_ready` for a given reference --
    /// standing in for CloudNativePG bootstrapping a cluster over a handful
    /// of polls, without a unit test actually waiting on one.
    struct FakeInstances {
        state: Mutex<HashMap<IdentityInstanceRef, DesiredIdentityInstance>>,
        deleted_namespaces: Mutex<Vec<String>>,
        ready_after: u32,
        polls: Mutex<u32>,
        database_uri: String,
        fail_database_uri: bool,
        fail_delete_namespace: bool,
    }

    impl FakeInstances {
        fn ready_immediately() -> Self {
            Self {
                state: Mutex::new(HashMap::new()),
                deleted_namespaces: Mutex::new(Vec::new()),
                ready_after: 0,
                polls: Mutex::new(0),
                database_uri: "postgres://drill:drill@localhost/drill".to_string(),
                fail_database_uri: false,
                fail_delete_namespace: false,
            }
        }

        fn never_ready() -> Self {
            Self {
                ready_after: u32::MAX,
                ..Self::ready_immediately()
            }
        }
    }

    impl IdentityInstancePort for FakeInstances {
        fn apply<'a>(
            &'a self,
            desired: &'a DesiredIdentityInstance,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            Box::pin(async move {
                self.state
                    .lock()
                    .unwrap()
                    .insert(desired.reference.clone(), desired.clone());
                Ok(())
            })
        }

        fn delete<'a>(
            &'a self,
            _reference: &'a IdentityInstanceRef,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            Box::pin(async { Ok(()) })
        }

        fn set_allowed_cidrs<'a>(
            &'a self,
            _reference: &'a IdentityInstanceRef,
            _ranges: Option<Vec<String>>,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            Box::pin(async { Ok(()) })
        }

        fn take_archive<'a>(
            &'a self,
            _reference: &'a IdentityInstanceRef,
            _name: &'a str,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            Box::pin(async { Ok(()) })
        }

        fn is_ready<'a>(
            &'a self,
            _reference: &'a IdentityInstanceRef,
        ) -> BoxFuture<'a, Result<bool, GenesisError>> {
            Box::pin(async move {
                let mut polls = self.polls.lock().unwrap();
                *polls += 1;
                Ok(*polls > self.ready_after)
            })
        }

        fn database_uri<'a>(
            &'a self,
            _reference: &'a IdentityInstanceRef,
        ) -> BoxFuture<'a, Result<String, GenesisError>> {
            Box::pin(async move {
                if self.fail_database_uri {
                    return Err(GenesisError::Kubernetes {
                        message: "no such secret".to_string(),
                    });
                }
                Ok(self.database_uri.clone())
            })
        }

        fn delete_namespace<'a>(
            &'a self,
            namespace: &'a str,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            Box::pin(async move {
                if self.fail_delete_namespace {
                    return Err(GenesisError::Kubernetes {
                        message: "namespace stuck terminating".to_string(),
                    });
                }
                self.deleted_namespaces
                    .lock()
                    .unwrap()
                    .push(namespace.to_string());
                Ok(())
            })
        }
    }

    /// Stands in for the archive: `Ok` is a database that answers, `Err` is
    /// the deliberately corrupted archive the acceptance criteria call for.
    struct FakeDatabase {
        answers: bool,
    }

    impl DatabaseProbe for FakeDatabase {
        fn answers_a_query<'a>(&'a self, _uri: &'a str) -> BoxFuture<'a, Result<(), GenesisError>> {
            Box::pin(async move {
                if self.answers {
                    Ok(())
                } else {
                    Err(GenesisError::Internal {
                        message: "the restored database did not answer".to_string(),
                    })
                }
            })
        }
    }

    #[derive(Default)]
    struct FakeOutcomes {
        published: Mutex<Vec<DeploymentOutcomeReport>>,
    }

    impl OutcomePublisher for FakeOutcomes {
        fn publish<'a>(
            &'a self,
            report: DeploymentOutcomeReport,
        ) -> BoxFuture<'a, Result<(), GenesisError>> {
            self.published.lock().unwrap().push(report);
            Box::pin(async { Ok(()) })
        }
    }

    fn drill_event() -> ActionEvent {
        ActionEvent {
            action_id: ACTION,
            deployment_id: DEPLOYMENT,
            dataplane_id: Uuid::from_u128(3),
            routing_key: "deployment.drill".to_string(),
            version: 1,
            payload: json!({
                "deployment_id": DEPLOYMENT,
                "organisation_id": ORGANISATION,
                "kind": "ferriskey",
                "version": "26.0.1",
                "cpu_millis": 500,
                "memory_mib": 1024,
                "storage_gib": 5,
                "source": {
                    "destination_path": "s3://aether-backups/an-org/a-deployment",
                    "server_name": "deployment-1-db",
                    "backup_id": "an-archive",
                },
            }),
            occurred_at: Utc::now(),
        }
    }

    #[test]
    fn answers_to_the_drill_routing_key() {
        let handler = DrillEventHandler::new(
            Arc::new(FakeInstances::ready_immediately()),
            Arc::new(FakeDatabase { answers: true }),
            Arc::new(FakeOutcomes::default()),
        );

        assert_eq!(handler.routing_key(), "deployment.drill");
    }

    #[tokio::test]
    async fn a_successful_drill_reports_success_with_a_duration() {
        let outcomes = Arc::new(FakeOutcomes::default());
        let handler = DrillEventHandler::new(
            Arc::new(FakeInstances::ready_immediately()),
            Arc::new(FakeDatabase { answers: true }),
            outcomes.clone(),
        );

        handler.handle(drill_event()).await.expect("handled");

        let published = outcomes.published.lock().unwrap();
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].deployment_id, DEPLOYMENT);
        assert_eq!(published[0].outcome, "drill_succeeded");
        assert!(published[0].duration_seconds.is_some());
    }

    /// The literal acceptance criterion of #185: a deliberately corrupted
    /// archive makes the drill fail rather than pass quietly.
    #[tokio::test]
    async fn a_database_that_does_not_answer_fails_the_drill_not_the_handler() {
        let outcomes = Arc::new(FakeOutcomes::default());
        let handler = DrillEventHandler::new(
            Arc::new(FakeInstances::ready_immediately()),
            Arc::new(FakeDatabase { answers: false }),
            outcomes.clone(),
        );

        let result = handler.handle(drill_event()).await;

        assert!(result.is_ok(), "a failed drill is a result, not a retry");
        let published = outcomes.published.lock().unwrap();
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].outcome, "drill_failed");
        assert!(published[0].reason.is_some());
        assert!(published[0].duration_seconds.is_some());
    }

    /// An instance that never comes up -- the shape a corrupted archive
    /// takes when CloudNativePG cannot bootstrap from it at all -- fails the
    /// drill the same way a database that refuses a query does.
    #[tokio::test]
    async fn an_instance_that_never_becomes_ready_fails_the_drill() {
        let outcomes = Arc::new(FakeOutcomes::default());
        let handler = DrillEventHandler::new(
            Arc::new(FakeInstances::never_ready()),
            Arc::new(FakeDatabase { answers: true }),
            outcomes.clone(),
        )
        .with_timing(Duration::from_millis(30), Duration::from_millis(5));

        let result = handler.handle(drill_event()).await;

        assert!(result.is_ok());
        let published = outcomes.published.lock().unwrap();
        assert_eq!(published[0].outcome, "drill_failed");
    }

    #[tokio::test]
    async fn the_namespace_is_removed_after_a_successful_drill() {
        let instances = Arc::new(FakeInstances::ready_immediately());
        let handler = DrillEventHandler::new(
            instances.clone(),
            Arc::new(FakeDatabase { answers: true }),
            Arc::new(FakeOutcomes::default()),
        );

        handler.handle(drill_event()).await.expect("handled");

        assert_eq!(
            instances.deleted_namespaces.lock().unwrap().clone(),
            vec![format!("drill-{ACTION}")]
        );
    }

    /// The namespace is what a failed drill would otherwise leave consuming
    /// real capacity on a real data plane -- torn down whether the drill
    /// passed or not.
    #[tokio::test]
    async fn the_namespace_is_removed_after_a_failed_drill_too() {
        let instances = Arc::new(FakeInstances::ready_immediately());
        let handler = DrillEventHandler::new(
            instances.clone(),
            Arc::new(FakeDatabase { answers: false }),
            Arc::new(FakeOutcomes::default()),
        );

        handler.handle(drill_event()).await.expect("handled");

        assert_eq!(instances.deleted_namespaces.lock().unwrap().len(), 1);
    }

    /// A read that cannot happen -- the connection secret CloudNativePG was
    /// supposed to have written is not there -- is treated as the drill
    /// failing to verify, not as a processing error to retry: the instance
    /// really did come up, just not into a state the drill can read.
    #[tokio::test]
    async fn an_unreadable_connection_secret_fails_the_drill_not_the_handler() {
        let mut instances = FakeInstances::ready_immediately();
        instances.fail_database_uri = true;
        let outcomes = Arc::new(FakeOutcomes::default());
        let handler = DrillEventHandler::new(
            Arc::new(instances),
            Arc::new(FakeDatabase { answers: true }),
            outcomes.clone(),
        );

        let result = handler.handle(drill_event()).await;

        assert!(result.is_ok());
        assert_eq!(
            outcomes.published.lock().unwrap()[0].outcome,
            "drill_failed"
        );
    }

    /// A namespace that cannot be removed is retried, not reported: reporting
    /// success or failure while the namespace is still there would tell the
    /// control plane the drill is done when it has not given back the
    /// capacity it borrowed.
    #[tokio::test]
    async fn a_namespace_that_cannot_be_removed_is_retried_rather_than_reported() {
        let mut instances = FakeInstances::ready_immediately();
        instances.fail_delete_namespace = true;
        let outcomes = Arc::new(FakeOutcomes::default());
        let handler = DrillEventHandler::new(
            Arc::new(instances),
            Arc::new(FakeDatabase { answers: true }),
            outcomes.clone(),
        );

        let result = handler.handle(drill_event()).await;

        assert!(result.is_err(), "the consumer must requeue this");
        assert!(outcomes.published.lock().unwrap().is_empty());
    }

    /// Redelivery of the same action must target the same throwaway
    /// namespace -- the mandatory idempotency requirement every handler in
    /// this crate is held to.
    #[test]
    fn redelivery_of_the_same_action_targets_the_same_namespace() {
        let a = DrillEventHandler::reference(ACTION);
        let b = DrillEventHandler::reference(ACTION);

        assert_eq!(a, b);
        assert!(
            !a.name.starts_with("deployment-"),
            "the global status watcher must never mistake this for a real deployment"
        );
    }

    #[test]
    fn an_unknown_kind_is_refused_before_anything_is_applied() {
        let mut payload = drill_event().payload;
        payload["kind"] = json!("bogus");
        let payload = DrillPayloadV1::from_value(&payload).expect("still a valid shape");
        let reference = DrillEventHandler::reference(ACTION);

        assert!(DrillEventHandler::desired(&payload, &reference).is_err());
    }
}
