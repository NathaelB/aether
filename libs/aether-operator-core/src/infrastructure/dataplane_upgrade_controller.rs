use std::env;
use std::sync::Arc;
use std::time::Duration;

use aether_crds::v1alpha::identity_dataplane_upgrade::{
    IdentityDataplaneUpgrade, IdentityDataplaneUpgradeSpec, IdentityDataplaneUpgradeStatus,
};
use chrono::Utc;
use futures::StreamExt;
use kube::runtime::controller::{Action, Controller};
use kube::runtime::watcher;
use kube::{Api, Client};
use tracing::{error, info, warn};

use crate::domain::OperatorError;
use crate::domain::dataplane_upgrade::DataplaneComponentKind;
use crate::domain::dataplane_upgrade::service::{self, Failure, Observed, Step};
use crate::domain::ports::{DataplaneUpgradeDeployer, DataplaneUpgradeRepository};
use crate::infrastructure::dataplane_upgrade::{
    KubeDataplaneUpgradeDeployer, KubeDataplaneUpgradeRepository,
};

pub const READY_TIMEOUT_ENV: &str = "DATAPLANE_UPGRADE_READY_TIMEOUT_SECONDS";
pub const RELEASE_NAME_ENV: &str = "AETHER_RELEASE_NAME";
const DEFAULT_READY_TIMEOUT: Duration = Duration::from_secs(300);
const POLL_INTERVAL: Duration = Duration::from_secs(5);
const ERROR_RETRY: Duration = Duration::from_secs(30);

pub fn ready_timeout(value: Option<&str>) -> Duration {
    value
        .and_then(|seconds| seconds.trim().parse::<u64>().ok())
        .map_or(DEFAULT_READY_TIMEOUT, Duration::from_secs)
}

struct Context<R, D> {
    repository: R,
    deployer: D,
    timeout: Duration,
}

pub async fn run() -> Result<(), OperatorError> {
    info!("Starting IdentityDataplaneUpgrade controller");
    let client = Client::try_default()
        .await
        .map_err(|error| OperatorError::Kube {
            message: error.to_string(),
        })?;

    let release = env::var(RELEASE_NAME_ENV)
        .ok()
        .filter(|name| !name.is_empty());
    let timeout = ready_timeout(env::var(READY_TIMEOUT_ENV).ok().as_deref());
    if release.is_none() {
        warn!("{RELEASE_NAME_ENV} is not set: components are matched by role label alone");
    }

    let upgrades = Api::<IdentityDataplaneUpgrade>::all(client.clone());
    let context = Arc::new(Context {
        repository: KubeDataplaneUpgradeRepository::new(client.clone()),
        deployer: KubeDataplaneUpgradeDeployer::new(client, release),
        timeout,
    });

    Controller::new(upgrades, watcher::Config::default())
        .run(
            reconcile::<KubeDataplaneUpgradeRepository, KubeDataplaneUpgradeDeployer>,
            error_policy::<KubeDataplaneUpgradeRepository, KubeDataplaneUpgradeDeployer>,
            context,
        )
        .for_each(|_| async {})
        .await;

    Ok(())
}

async fn reconcile<R, D>(
    upgrade: Arc<IdentityDataplaneUpgrade>,
    context: Arc<Context<R, D>>,
) -> Result<Action, OperatorError>
where
    R: DataplaneUpgradeRepository,
    D: DataplaneUpgradeDeployer,
{
    drive(
        &upgrade,
        &context.repository,
        &context.deployer,
        context.timeout,
    )
    .await
}

fn error_policy<R, D>(
    _upgrade: Arc<IdentityDataplaneUpgrade>,
    error: &OperatorError,
    _context: Arc<Context<R, D>>,
) -> Action {
    error!(error = %error, "IdentityDataplaneUpgrade reconcile error");
    Action::requeue(ERROR_RETRY)
}

async fn drive<R, D>(
    upgrade: &IdentityDataplaneUpgrade,
    repository: &R,
    deployer: &D,
    timeout: Duration,
) -> Result<Action, OperatorError>
where
    R: DataplaneUpgradeRepository,
    D: DataplaneUpgradeDeployer,
{
    let name = upgrade
        .metadata
        .name
        .as_deref()
        .ok_or(OperatorError::MissingName)?;
    let namespace =
        upgrade
            .metadata
            .namespace
            .as_deref()
            .ok_or_else(|| OperatorError::MissingNamespace {
                name: name.to_string(),
            })?;
    let spec = &upgrade.spec;
    let mut status = upgrade.status.clone().unwrap_or_default();

    loop {
        let step = match service::preflight(spec, &status) {
            Some(step) => step,
            None => {
                let observed = observe(deployer, namespace, spec).await?;
                service::decide(spec, &status, &observed, timeout)
            }
        };
        info!(name, namespace, ?step, "IdentityDataplaneUpgrade step");

        match &step {
            Step::Idle => return Ok(Action::await_change()),
            Step::Wait => return Ok(Action::requeue(POLL_INTERVAL)),
            Step::Patch(component) => {
                deployer
                    .set_component_version(namespace, *component, &spec.target_version)
                    .await?;
                return Ok(Action::requeue(POLL_INTERVAL));
            }
            Step::Start { component, .. } => {
                status = record(repository, name, namespace, spec, &status, &step).await?;
                deployer
                    .set_component_version(namespace, *component, &spec.target_version)
                    .await?;
            }
            Step::RollBack { restore, .. } => {
                let step = match restore_all(deployer, namespace, restore).await {
                    Ok(()) => step,
                    Err(failure) => Step::Fail(failure),
                };
                status = record(repository, name, namespace, spec, &status, &step).await?;
            }
            Step::MarkUpgraded(_) | Step::Complete | Step::Fail(_) => {
                status = record(repository, name, namespace, spec, &status, &step).await?;
            }
        }
    }
}

async fn record<R: DataplaneUpgradeRepository>(
    repository: &R,
    name: &str,
    namespace: &str,
    spec: &IdentityDataplaneUpgradeSpec,
    status: &IdentityDataplaneUpgradeStatus,
    step: &Step,
) -> Result<IdentityDataplaneUpgradeStatus, OperatorError> {
    let next = service::advance(spec, status, step, Utc::now());
    repository
        .patch_status(name, namespace, next.clone())
        .await?;
    Ok(next)
}

async fn restore_all<D: DataplaneUpgradeDeployer>(
    deployer: &D,
    namespace: &str,
    restore: &[(DataplaneComponentKind, String)],
) -> Result<(), Failure> {
    for (component, version) in restore {
        deployer
            .set_component_version(namespace, *component, version)
            .await
            .map_err(|error| Failure::RollbackFailed {
                component: *component,
                message: error.to_string(),
            })?;
    }
    Ok(())
}

async fn observe<D: DataplaneUpgradeDeployer>(
    deployer: &D,
    namespace: &str,
    spec: &IdentityDataplaneUpgradeSpec,
) -> Result<Observed, OperatorError> {
    let versions = deployer.current_versions(namespace).await?;
    let mut ready = std::collections::BTreeMap::new();
    for component in DataplaneComponentKind::expand(&spec.components) {
        ready.insert(
            component,
            deployer.component_ready(namespace, component).await?,
        );
    }
    Ok(Observed {
        versions,
        ready,
        now: Utc::now(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use aether_crds::v1alpha::identity_dataplane_upgrade::{
        ComponentUpgradeState, ComponentUpgradeStatus, DataplaneComponent, DataplaneUpgradePhase,
        DataplaneUpgradeStrategy, IdentityDataplaneUpgradeSpec, IdentityDataplaneUpgradeStatus,
    };

    use super::*;
    use crate::domain::dataplane_upgrade::ComponentVersions;
    use crate::domain::ports::{MockDataplaneUpgradeDeployer, MockDataplaneUpgradeRepository};

    use DataplaneComponentKind::{Genesis, Herald, Operator};

    const OLD: &str = "dpu-4";
    const TARGET: &str = "dpu-5";
    const LONG: Duration = Duration::from_secs(3600);

    #[derive(Default)]
    struct Cluster {
        versions: ComponentVersions,
        ready: BTreeMap<DataplaneComponentKind, bool>,
        sets: Vec<(DataplaneComponentKind, String)>,
        statuses: Vec<IdentityDataplaneUpgradeStatus>,
        fail_restores: bool,
    }

    type Shared = Arc<Mutex<Cluster>>;

    fn cluster(versions: &[(DataplaneComponentKind, &str)], ready: &[bool; 3]) -> Shared {
        let cluster = Cluster {
            versions: versions
                .iter()
                .map(|(component, version)| (*component, version.to_string()))
                .collect(),
            ready: DataplaneComponentKind::ALL
                .into_iter()
                .zip(ready.iter().copied())
                .collect(),
            ..Default::default()
        };
        Arc::new(Mutex::new(cluster))
    }

    fn old_cluster(ready: &[bool; 3]) -> Shared {
        cluster(&[(Herald, OLD), (Genesis, OLD), (Operator, OLD)], ready)
    }

    fn ports(shared: &Shared) -> (MockDataplaneUpgradeRepository, MockDataplaneUpgradeDeployer) {
        let mut repository = MockDataplaneUpgradeRepository::new();
        let mut deployer = MockDataplaneUpgradeDeployer::new();

        let state = Arc::clone(shared);
        deployer.expect_current_versions().returning(move |_| {
            let versions = state.lock().unwrap().versions.clone();
            Box::pin(async move { Ok(versions) })
        });

        let state = Arc::clone(shared);
        deployer
            .expect_component_ready()
            .returning(move |_, component| {
                let ready = state.lock().unwrap().ready[&component];
                Box::pin(async move { Ok(ready) })
            });

        let state = Arc::clone(shared);
        deployer
            .expect_set_component_version()
            .returning(move |_, component, version| {
                let mut cluster = state.lock().unwrap();
                let result = if version == OLD && cluster.fail_restores {
                    Err(OperatorError::Kube {
                        message: "apiserver unreachable".to_string(),
                    })
                } else {
                    if version == TARGET {
                        let recorded = cluster.statuses.last().and_then(|status| {
                            status
                                .components
                                .iter()
                                .find(|entry| entry.name == DataplaneComponent::from(component))
                        });
                        let recorded = recorded.expect("status written before the patch");
                        assert_eq!(recorded.state, ComponentUpgradeState::Upgrading);
                        assert_eq!(recorded.previous_version.as_deref(), Some(OLD));
                        assert!(recorded.started_at.is_some());
                    }
                    cluster.versions.insert(component, version.to_string());
                    cluster.sets.push((component, version.to_string()));
                    Ok(())
                };
                Box::pin(async move { result })
            });

        let state = Arc::clone(shared);
        repository
            .expect_patch_status()
            .returning(move |_, _, status| {
                state.lock().unwrap().statuses.push(status);
                Box::pin(async { Ok(()) })
            });

        (repository, deployer)
    }

    fn upgrade(
        strategy: DataplaneUpgradeStrategy,
        status: Option<IdentityDataplaneUpgradeStatus>,
    ) -> IdentityDataplaneUpgrade {
        let mut upgrade = IdentityDataplaneUpgrade::new(
            "dpu",
            IdentityDataplaneUpgradeSpec {
                dataplane_id: "dp".to_string(),
                target_version: TARGET.to_string(),
                components: vec![DataplaneComponent::All],
                strategy,
                max_unavailable: 1,
            },
        );
        upgrade.metadata.namespace = Some("aether".to_string());
        upgrade.status = status;
        upgrade
    }

    fn last_status(shared: &Shared) -> IdentityDataplaneUpgradeStatus {
        shared.lock().unwrap().statuses.last().unwrap().clone()
    }

    fn sets(shared: &Shared) -> Vec<(DataplaneComponentKind, String)> {
        shared.lock().unwrap().sets.clone()
    }

    fn entry(
        component: DataplaneComponentKind,
        state: ComponentUpgradeState,
    ) -> ComponentUpgradeStatus {
        ComponentUpgradeStatus {
            name: component.into(),
            previous_version: Some(OLD.to_string()),
            state,
            started_at: Some("2026-01-01T00:00:00Z".to_string()),
        }
    }

    #[tokio::test]
    async fn rolling_upgrade_moves_components_one_at_a_time_operator_last() {
        let shared = old_cluster(&[true, true, true]);
        let (repository, deployer) = ports(&shared);
        let upgrade = upgrade(DataplaneUpgradeStrategy::Rolling, None);

        let action = drive(&upgrade, &repository, &deployer, LONG).await.unwrap();

        assert_eq!(action, Action::await_change());
        assert_eq!(
            sets(&shared),
            vec![
                (Herald, TARGET.to_string()),
                (Genesis, TARGET.to_string()),
                (Operator, TARGET.to_string()),
            ]
        );
        let status = last_status(&shared);
        assert_eq!(status.phase, DataplaneUpgradePhase::Completed);
        assert_eq!(status.current_version.as_deref(), Some(TARGET));
        assert_eq!(status.progress.as_deref(), Some("3/3 components updated"));
    }

    #[tokio::test]
    async fn waiting_for_readiness_requeues_without_touching_the_next_component() {
        let shared = old_cluster(&[false, true, true]);
        let (repository, deployer) = ports(&shared);
        let upgrade = upgrade(DataplaneUpgradeStrategy::Rolling, None);

        let action = drive(&upgrade, &repository, &deployer, LONG).await.unwrap();

        assert_eq!(action, Action::requeue(POLL_INTERVAL));
        assert_eq!(sets(&shared), vec![(Herald, TARGET.to_string())]);
        assert_eq!(last_status(&shared).phase, DataplaneUpgradePhase::Upgrading);
    }

    #[tokio::test]
    async fn component_never_ready_restores_previous_versions_in_reverse() {
        let shared = old_cluster(&[true, true, false]);
        let (repository, deployer) = ports(&shared);
        let upgrade = upgrade(DataplaneUpgradeStrategy::Rolling, None);

        let action = drive(&upgrade, &repository, &deployer, Duration::ZERO)
            .await
            .unwrap();

        assert_eq!(action, Action::await_change());
        assert_eq!(
            sets(&shared),
            vec![
                (Herald, TARGET.to_string()),
                (Genesis, TARGET.to_string()),
                (Operator, TARGET.to_string()),
                (Operator, OLD.to_string()),
                (Genesis, OLD.to_string()),
                (Herald, OLD.to_string()),
            ]
        );
        let status = last_status(&shared);
        assert_eq!(status.phase, DataplaneUpgradePhase::RolledBack);
        assert_eq!(status.current_version, None);
        assert_eq!(status.conditions[0].reason.as_deref(), Some("ReadyTimeout"));
    }

    #[tokio::test]
    async fn failing_rollback_ends_failed_with_the_reason() {
        let shared = old_cluster(&[true, true, false]);
        shared.lock().unwrap().fail_restores = true;
        let (repository, deployer) = ports(&shared);
        let upgrade = upgrade(DataplaneUpgradeStrategy::Rolling, None);

        drive(&upgrade, &repository, &deployer, Duration::ZERO)
            .await
            .unwrap();

        let status = last_status(&shared);
        assert_eq!(status.phase, DataplaneUpgradePhase::Failed);
        assert_eq!(
            status.conditions[0].reason.as_deref(),
            Some("RollbackFailed")
        );
        assert!(
            status.conditions[0]
                .message
                .as_deref()
                .unwrap()
                .contains("apiserver unreachable")
        );
    }

    #[tokio::test]
    async fn restarted_reconciler_resumes_at_the_recorded_component() {
        let shared = cluster(
            &[(Herald, TARGET), (Genesis, TARGET), (Operator, OLD)],
            &[true, true, true],
        );
        let (repository, deployer) = ports(&shared);
        let recorded = IdentityDataplaneUpgradeStatus {
            phase: DataplaneUpgradePhase::Upgrading,
            components: vec![
                entry(Herald, ComponentUpgradeState::Upgraded),
                entry(Genesis, ComponentUpgradeState::Upgraded),
            ],
            ..Default::default()
        };
        let upgrade = upgrade(DataplaneUpgradeStrategy::Rolling, Some(recorded));

        drive(&upgrade, &repository, &deployer, LONG).await.unwrap();

        assert_eq!(sets(&shared), vec![(Operator, TARGET.to_string())]);
        assert_eq!(last_status(&shared).phase, DataplaneUpgradePhase::Completed);
    }

    #[tokio::test]
    async fn operator_already_on_target_and_ready_completes_without_patching() {
        let shared = cluster(
            &[(Herald, TARGET), (Genesis, TARGET), (Operator, TARGET)],
            &[true, true, true],
        );
        let (repository, deployer) = ports(&shared);
        let recorded = IdentityDataplaneUpgradeStatus {
            phase: DataplaneUpgradePhase::Upgrading,
            components: vec![
                entry(Herald, ComponentUpgradeState::Upgraded),
                entry(Genesis, ComponentUpgradeState::Upgraded),
                entry(Operator, ComponentUpgradeState::Upgrading),
            ],
            ..Default::default()
        };
        let upgrade = upgrade(DataplaneUpgradeStrategy::Rolling, Some(recorded));

        drive(&upgrade, &repository, &deployer, LONG).await.unwrap();

        assert!(sets(&shared).is_empty());
        let status = last_status(&shared);
        assert_eq!(status.phase, DataplaneUpgradePhase::Completed);
        assert_eq!(status.current_version.as_deref(), Some(TARGET));
    }

    #[tokio::test]
    async fn terminal_upgrade_is_not_touched_again() {
        for phase in [
            DataplaneUpgradePhase::Completed,
            DataplaneUpgradePhase::Failed,
            DataplaneUpgradePhase::RolledBack,
        ] {
            let repository = MockDataplaneUpgradeRepository::new();
            let deployer = MockDataplaneUpgradeDeployer::new();
            let status = IdentityDataplaneUpgradeStatus {
                phase,
                ..Default::default()
            };
            let upgrade = upgrade(DataplaneUpgradeStrategy::Rolling, Some(status));

            let action = drive(&upgrade, &repository, &deployer, LONG).await.unwrap();

            assert_eq!(action, Action::await_change());
        }
    }

    #[tokio::test]
    async fn canary_fails_without_reading_or_changing_any_deployment() {
        let shared = old_cluster(&[true, true, true]);
        let (repository, _) = ports(&shared);
        let deployer = MockDataplaneUpgradeDeployer::new();
        let upgrade = upgrade(DataplaneUpgradeStrategy::Canary, None);

        let action = drive(&upgrade, &repository, &deployer, LONG).await.unwrap();

        assert_eq!(action, Action::await_change());
        let status = last_status(&shared);
        assert_eq!(status.phase, DataplaneUpgradePhase::Failed);
        assert_eq!(status.conditions[0].condition_type, "Accepted");
        assert_eq!(
            status.conditions[0].reason.as_deref(),
            Some("CanaryNotSupported")
        );
        assert!(sets(&shared).is_empty());
    }

    #[test]
    fn ready_timeout_defaults_to_five_minutes() {
        assert_eq!(ready_timeout(None), Duration::from_secs(300));
        assert_eq!(ready_timeout(Some("soon")), Duration::from_secs(300));
        assert_eq!(ready_timeout(Some("")), Duration::from_secs(300));
    }

    #[test]
    fn ready_timeout_reads_seconds() {
        assert_eq!(ready_timeout(Some("42")), Duration::from_secs(42));
        assert_eq!(ready_timeout(Some(" 90 ")), Duration::from_secs(90));
    }
}
