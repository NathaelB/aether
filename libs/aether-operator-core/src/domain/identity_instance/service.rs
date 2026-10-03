use std::sync::Arc;
use std::time::Duration;

use aether_crds::common::types::Phase;
use aether_crds::v1alpha::identity_instance::{
    IdentityInstance, IdentityInstanceStatus, IdentityProvider,
};

use crate::domain::identity_instance::theme::{
    ThemeDecision, decide, default_theme_config, differs, marker, outcome_status, theme_config,
};
use crate::domain::ports::{
    IdentityInstanceDeployer, IdentityInstanceRepository, IdentityInstanceService,
    InstanceThemePort,
};
use crate::domain::{OperatorError, ReconcileOutcome};

const DEPLOYING_REQUEUE_SECONDS: u64 = 15;
const STEADY_STATE_REQUEUE_SECONDS: u64 = 60;

pub struct IdentityInstanceServiceImpl<R, D, T> {
    repository: Arc<R>,
    deployer: Arc<D>,
    theme: Arc<T>,
}

impl<R, D, T> IdentityInstanceServiceImpl<R, D, T> {
    pub fn new(repository: Arc<R>, deployer: Arc<D>, theme: Arc<T>) -> Self {
        Self {
            repository,
            deployer,
            theme,
        }
    }

    fn build_desired_status(
        &self,
        instance: &IdentityInstance,
        database_ready: bool,
        provider_ready: bool,
        edge_ready: bool,
        upgrade_in_progress: bool,
    ) -> IdentityInstanceStatus {
        let mut status = instance.status.clone().unwrap_or_default();

        status.ready = database_ready && provider_ready && edge_ready && !upgrade_in_progress;
        status.phase = Some(if !database_ready {
            Phase::DatabaseProvisioning
        } else if upgrade_in_progress {
            Phase::Upgrading
        } else if provider_ready && edge_ready {
            Phase::Running
        } else {
            Phase::Deploying
        });

        if status.endpoint.is_none() {
            status.endpoint = Some(format!("https://{}", instance.spec.hostname));
        }

        if status.admin_url.is_none() {
            status.admin_url = Some(format!("https://{}/admin", instance.spec.hostname));
        }

        status
    }
}

impl<R, D, T> IdentityInstanceService for IdentityInstanceServiceImpl<R, D, T>
where
    R: IdentityInstanceRepository,
    D: IdentityInstanceDeployer,
    T: InstanceThemePort,
{
    async fn reconcile(
        &self,
        instance: IdentityInstance,
    ) -> Result<ReconcileOutcome, OperatorError> {
        self.ensure_instance(&instance).await?;
        let database_ready = self.deployer.database_ready(&instance).await?;
        let provider_ready = self.deployer.provider_ready(&instance).await?;
        let edge_ready = self.deployer.edge_ready(&instance).await?;
        let upgrade_in_progress = self.deployer.upgrade_in_progress(&instance).await?;
        let current_status = instance.status.clone().unwrap_or_default();
        let desired_status = self.build_desired_status(
            &instance,
            database_ready,
            provider_ready,
            edge_ready,
            upgrade_in_progress,
        );

        if desired_status != current_status {
            self.repository
                .patch_status(&instance, desired_status)
                .await?;
            return Ok(ReconcileOutcome::requeue_after(Duration::from_secs(
                DEPLOYING_REQUEUE_SECONDS,
            )));
        }

        if !database_ready || !provider_ready || !edge_ready || upgrade_in_progress {
            return Ok(ReconcileOutcome::requeue_after(Duration::from_secs(
                DEPLOYING_REQUEUE_SECONDS,
            )));
        }

        if self.reconcile_branding(&instance).await? == BrandingOutcome::Retry {
            return Ok(ReconcileOutcome::requeue_after(Duration::from_secs(
                DEPLOYING_REQUEUE_SECONDS,
            )));
        }

        Ok(ReconcileOutcome::requeue_after(Duration::from_secs(
            STEADY_STATE_REQUEUE_SECONDS,
        )))
    }
}

#[derive(Debug, PartialEq, Eq)]
enum BrandingOutcome {
    Settled,
    Retry,
}

impl<R, D, T> IdentityInstanceServiceImpl<R, D, T>
where
    R: IdentityInstanceRepository,
    T: InstanceThemePort,
{
    async fn reconcile_branding(
        &self,
        instance: &IdentityInstance,
    ) -> Result<BrandingOutcome, OperatorError> {
        if instance.spec.provider != IdentityProvider::Ferriskey {
            return Ok(BrandingOutcome::Settled);
        }

        let previous = instance
            .status
            .as_ref()
            .and_then(|status| status.iam.as_ref());
        let wanted = instance
            .spec
            .iam
            .as_ref()
            .and_then(|iam| iam.branding.as_ref());

        let (config, target) = match decide(wanted, previous.and_then(|iam| iam.applied.as_deref()))
        {
            ThemeDecision::Nothing => return Ok(BrandingOutcome::Settled),
            ThemeDecision::Apply(branding) => (theme_config(branding), Some(marker(branding))),
            ThemeDecision::RestoreDefault => (default_theme_config(), None),
        };

        let outcome = self.theme.put_theme(instance, &config).await;
        let retry = matches!(
            outcome,
            Err(crate::domain::identity_instance::theme::ThemeError::Retry { .. })
        );
        let next = outcome_status(previous, target, outcome, chrono::Utc::now().to_rfc3339());

        if differs(previous, &next) {
            let mut status = instance.status.clone().unwrap_or_default();
            status.iam = Some(next);
            self.repository.patch_status(instance, status).await?;
        }

        Ok(if retry {
            BrandingOutcome::Retry
        } else {
            BrandingOutcome::Settled
        })
    }
}

impl<R, D, T> IdentityInstanceServiceImpl<R, D, T>
where
    D: IdentityInstanceDeployer,
{
    async fn ensure_instance(&self, instance: &IdentityInstance) -> Result<(), OperatorError> {
        self.deployer.ensure_provider_resources(instance).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::identity_instance::theme::ThemeError;
    use crate::domain::ports::{
        MockIdentityInstanceDeployer, MockIdentityInstanceRepository, MockInstanceThemePort,
    };
    use aether_crds::common::types::Phase;
    use aether_crds::common::types::ResourceRequirements;
    use aether_crds::v1alpha::identity_instance::{
        Branding, DatabaseConfig, DatabaseMode, IamConfig, IamPhase, IamStatus, IdentityInstance,
        IdentityInstanceSpec, IdentityInstanceStatus, IdentityProvider, ManagedClusterConfig,
        ManagedClusterStorage,
    };
    use kube::core::ObjectMeta;
    use std::sync::Arc;

    fn instance_with_status(status: Option<IdentityInstanceStatus>) -> IdentityInstance {
        IdentityInstance {
            metadata: ObjectMeta {
                name: Some("instance-1".to_string()),
                namespace: Some("default".to_string()),
                ..Default::default()
            },
            spec: IdentityInstanceSpec {
                restore: None,
                organisation_id: "org-1".to_string(),
                provider: IdentityProvider::Keycloak,
                version: "25.0.0".to_string(),
                hostname: "auth.acme.test".to_string(),
                database: DatabaseConfig {
                    mode: DatabaseMode::ManagedCluster,
                    managed_cluster: ManagedClusterConfig {
                        instances: 1,
                        storage: ManagedClusterStorage {
                            size: "10Gi".to_string(),
                            storage_class: None,
                        },
                        resources: ResourceRequirements {
                            requests: None,
                            limits: None,
                        },
                    },
                },
                ferriskey: None,
                ingress: None,
                allowed_cidrs: None,
                iam: None,
                backup: None,
            },
            status,
        }
    }

    #[tokio::test]
    async fn reconcile_updates_status_and_requeues() {
        let instance = instance_with_status(None);
        let mut repository = MockIdentityInstanceRepository::new();
        let mut deployer = MockIdentityInstanceDeployer::new();

        deployer
            .expect_ensure_provider_resources()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        deployer
            .expect_database_ready()
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));
        deployer
            .expect_provider_ready()
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));
        deployer
            .expect_edge_ready()
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));
        deployer
            .expect_upgrade_in_progress()
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));

        repository
            .expect_patch_status()
            .times(1)
            .withf(|instance, status| {
                instance.metadata.name.as_deref() == Some("instance-1")
                    && status.phase == Some(Phase::DatabaseProvisioning)
                    && !status.ready
                    && status.endpoint.as_deref() == Some("https://auth.acme.test")
                    && status.admin_url.as_deref() == Some("https://auth.acme.test/admin")
            })
            .returning(|instance, _status| {
                let instance = instance.clone();
                Box::pin(async move { Ok(instance) })
            });

        let service = IdentityInstanceServiceImpl::new(
            Arc::new(repository),
            Arc::new(deployer),
            Arc::new(MockInstanceThemePort::new()),
        );
        let outcome = service.reconcile(instance).await.unwrap();

        assert_eq!(outcome.requeue_after, Some(Duration::from_secs(15)));
    }

    #[tokio::test]
    async fn reconcile_no_status_change_does_not_patch() {
        let status = IdentityInstanceStatus {
            phase: Some(Phase::Running),
            ready: true,
            endpoint: Some("https://auth.acme.test".to_string()),
            admin_url: Some("https://auth.acme.test/admin".to_string()),
            ..Default::default()
        };
        let instance = instance_with_status(Some(status));
        let mut repository = MockIdentityInstanceRepository::new();
        let mut deployer = MockIdentityInstanceDeployer::new();

        deployer
            .expect_ensure_provider_resources()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        deployer
            .expect_database_ready()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        deployer
            .expect_provider_ready()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        deployer
            .expect_edge_ready()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        deployer
            .expect_upgrade_in_progress()
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));
        repository.expect_patch_status().times(0);

        let service = IdentityInstanceServiceImpl::new(
            Arc::new(repository),
            Arc::new(deployer),
            Arc::new(MockInstanceThemePort::new()),
        );
        let outcome = service.reconcile(instance).await.unwrap();

        assert_eq!(
            outcome.requeue_after,
            Some(Duration::from_secs(STEADY_STATE_REQUEUE_SECONDS))
        );
    }

    #[tokio::test]
    async fn reconcile_requeues_while_provider_not_ready_even_without_status_change() {
        let status = IdentityInstanceStatus {
            phase: Some(Phase::Deploying),
            ready: false,
            endpoint: Some("https://auth.acme.test".to_string()),
            admin_url: Some("https://auth.acme.test/admin".to_string()),
            ..Default::default()
        };
        let instance = instance_with_status(Some(status));
        let mut repository = MockIdentityInstanceRepository::new();
        let mut deployer = MockIdentityInstanceDeployer::new();

        deployer
            .expect_ensure_provider_resources()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        deployer
            .expect_database_ready()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        deployer
            .expect_provider_ready()
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));
        deployer
            .expect_edge_ready()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        deployer
            .expect_upgrade_in_progress()
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));
        repository.expect_patch_status().times(0);

        let service = IdentityInstanceServiceImpl::new(
            Arc::new(repository),
            Arc::new(deployer),
            Arc::new(MockInstanceThemePort::new()),
        );
        let outcome = service.reconcile(instance).await.unwrap();

        assert_eq!(
            outcome.requeue_after,
            Some(Duration::from_secs(DEPLOYING_REQUEUE_SECONDS))
        );
    }

    #[test]
    fn build_desired_status_keeps_existing_fields() {
        let status = IdentityInstanceStatus {
            phase: Some(Phase::Running),
            ready: true,
            endpoint: Some("https://already.example".to_string()),
            admin_url: Some("https://already.example/admin".to_string()),
            ..Default::default()
        };
        let instance = instance_with_status(Some(status.clone()));
        let service = IdentityInstanceServiceImpl::new(
            Arc::new(MockIdentityInstanceRepository::new()),
            Arc::new(MockIdentityInstanceDeployer::new()),
            Arc::new(MockInstanceThemePort::new()),
        );

        let desired = service.build_desired_status(&instance, true, true, true, false);
        assert_eq!(desired.phase, status.phase);
        assert_eq!(desired.ready, status.ready);
        assert_eq!(desired.endpoint, status.endpoint);
        assert_eq!(desired.admin_url, status.admin_url);
    }

    fn ready_deployer() -> MockIdentityInstanceDeployer {
        let mut deployer = MockIdentityInstanceDeployer::new();
        deployer
            .expect_ensure_provider_resources()
            .returning(|_| Box::pin(async { Ok(()) }));
        deployer
            .expect_database_ready()
            .returning(|_| Box::pin(async { Ok(true) }));
        deployer
            .expect_provider_ready()
            .returning(|_| Box::pin(async { Ok(true) }));
        deployer
            .expect_edge_ready()
            .returning(|_| Box::pin(async { Ok(true) }));
        deployer
            .expect_upgrade_in_progress()
            .returning(|_| Box::pin(async { Ok(false) }));
        deployer
    }

    fn running_status(iam: Option<IamStatus>) -> IdentityInstanceStatus {
        IdentityInstanceStatus {
            phase: Some(Phase::Running),
            ready: true,
            endpoint: Some("https://auth.acme.test".to_string()),
            admin_url: Some("https://auth.acme.test/admin".to_string()),
            iam,
            ..Default::default()
        }
    }

    fn ferriskey_with(branding: Option<Branding>, iam: Option<IamStatus>) -> IdentityInstance {
        let mut instance = instance_with_status(Some(running_status(iam)));
        instance.spec.provider = IdentityProvider::Ferriskey;
        instance.spec.iam = Some(IamConfig { branding });
        instance
    }

    fn radius(radius: u8) -> Branding {
        Branding {
            colors: None,
            radius: Some(radius),
        }
    }

    fn iam_status(phase: IamPhase, applied: Option<String>) -> IamStatus {
        IamStatus {
            phase,
            message: None,
            applied,
            observed_at: Some("t".to_string()),
        }
    }

    #[tokio::test]
    async fn branding_not_yet_applied_is_applied_and_recorded() {
        let branding = radius(6);
        let instance = ferriskey_with(Some(branding.clone()), None);
        let mut theme = MockInstanceThemePort::new();
        theme
            .expect_put_theme()
            .times(1)
            .withf(|_, config| config["borders"]["widgetRadius"] == 6)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        let mut repository = MockIdentityInstanceRepository::new();
        let expected = marker(&branding);
        repository
            .expect_patch_status()
            .times(1)
            .withf(move |_, status| {
                status.iam.as_ref().is_some_and(|iam| {
                    iam.phase == IamPhase::Applied
                        && iam.applied.as_deref() == Some(expected.as_str())
                        && iam.message.is_none()
                })
            })
            .returning(|instance, _| {
                let instance = instance.clone();
                Box::pin(async move { Ok(instance) })
            });

        let service = IdentityInstanceServiceImpl::new(
            Arc::new(repository),
            Arc::new(ready_deployer()),
            Arc::new(theme),
        );
        let outcome = service.reconcile(instance).await.unwrap();

        assert_eq!(
            outcome.requeue_after,
            Some(Duration::from_secs(STEADY_STATE_REQUEUE_SECONDS))
        );
    }

    #[tokio::test]
    async fn unchanged_branding_is_not_applied_again() {
        let branding = radius(6);
        let applied = iam_status(IamPhase::Applied, Some(marker(&branding)));
        let instance = ferriskey_with(Some(branding), Some(applied));
        let mut repository = MockIdentityInstanceRepository::new();
        repository.expect_patch_status().times(0);

        let service = IdentityInstanceServiceImpl::new(
            Arc::new(repository),
            Arc::new(ready_deployer()),
            Arc::new(MockInstanceThemePort::new()),
        );
        service.reconcile(instance).await.unwrap();
    }

    #[tokio::test]
    async fn no_branding_and_no_marker_makes_no_call() {
        let instance = ferriskey_with(None, None);
        let mut repository = MockIdentityInstanceRepository::new();
        repository.expect_patch_status().times(0);

        let service = IdentityInstanceServiceImpl::new(
            Arc::new(repository),
            Arc::new(ready_deployer()),
            Arc::new(MockInstanceThemePort::new()),
        );
        service.reconcile(instance).await.unwrap();
    }

    #[tokio::test]
    async fn cleared_branding_restores_the_default_once() {
        let applied = iam_status(IamPhase::Applied, Some(marker(&radius(6))));
        let instance = ferriskey_with(None, Some(applied));
        let mut theme = MockInstanceThemePort::new();
        theme
            .expect_put_theme()
            .times(1)
            .withf(|_, config| *config == serde_json::json!({}))
            .returning(|_, _| Box::pin(async { Ok(()) }));
        let mut repository = MockIdentityInstanceRepository::new();
        repository
            .expect_patch_status()
            .times(1)
            .withf(|_, status| {
                status
                    .iam
                    .as_ref()
                    .is_some_and(|iam| iam.phase == IamPhase::Applied && iam.applied.is_none())
            })
            .returning(|instance, _| {
                let instance = instance.clone();
                Box::pin(async move { Ok(instance) })
            });

        let service = IdentityInstanceServiceImpl::new(
            Arc::new(repository),
            Arc::new(ready_deployer()),
            Arc::new(theme),
        );
        service.reconcile(instance).await.unwrap();
    }

    #[tokio::test]
    async fn unreachable_provider_is_pending_and_requeued_soon() {
        let instance = ferriskey_with(Some(radius(6)), None);
        let mut theme = MockInstanceThemePort::new();
        theme.expect_put_theme().times(1).returning(|_, _| {
            Box::pin(async {
                Err(ThemeError::Retry {
                    message: "FerrisKey is not answering".to_string(),
                })
            })
        });
        let mut repository = MockIdentityInstanceRepository::new();
        repository
            .expect_patch_status()
            .times(1)
            .withf(|_, status| {
                status.iam.as_ref().is_some_and(|iam| {
                    iam.phase == IamPhase::Pending
                        && iam.message.as_deref() == Some("FerrisKey is not answering")
                        && iam.applied.is_none()
                })
            })
            .returning(|instance, _| {
                let instance = instance.clone();
                Box::pin(async move { Ok(instance) })
            });

        let service = IdentityInstanceServiceImpl::new(
            Arc::new(repository),
            Arc::new(ready_deployer()),
            Arc::new(theme),
        );
        let outcome = service.reconcile(instance).await.unwrap();

        assert_eq!(
            outcome.requeue_after,
            Some(Duration::from_secs(DEPLOYING_REQUEUE_SECONDS))
        );
    }

    #[tokio::test]
    async fn repeated_pending_with_the_same_reason_does_not_patch_again() {
        let mut pending = iam_status(IamPhase::Pending, None);
        pending.message = Some("FerrisKey is not answering".to_string());
        let instance = ferriskey_with(Some(radius(6)), Some(pending));
        let mut theme = MockInstanceThemePort::new();
        theme.expect_put_theme().times(1).returning(|_, _| {
            Box::pin(async {
                Err(ThemeError::Retry {
                    message: "FerrisKey is not answering".to_string(),
                })
            })
        });
        let mut repository = MockIdentityInstanceRepository::new();
        repository.expect_patch_status().times(0);

        let service = IdentityInstanceServiceImpl::new(
            Arc::new(repository),
            Arc::new(ready_deployer()),
            Arc::new(theme),
        );
        service.reconcile(instance).await.unwrap();
    }

    #[tokio::test]
    async fn rejected_config_is_failed_with_the_message() {
        let instance = ferriskey_with(Some(radius(6)), None);
        let mut theme = MockInstanceThemePort::new();
        theme.expect_put_theme().times(1).returning(|_, _| {
            Box::pin(async {
                Err(ThemeError::Rejected {
                    message: "invalid color".to_string(),
                })
            })
        });
        let mut repository = MockIdentityInstanceRepository::new();
        repository
            .expect_patch_status()
            .times(1)
            .withf(|_, status| {
                status.iam.as_ref().is_some_and(|iam| {
                    iam.phase == IamPhase::Failed && iam.message.as_deref() == Some("invalid color")
                })
            })
            .returning(|instance, _| {
                let instance = instance.clone();
                Box::pin(async move { Ok(instance) })
            });

        let service = IdentityInstanceServiceImpl::new(
            Arc::new(repository),
            Arc::new(ready_deployer()),
            Arc::new(theme),
        );
        let outcome = service.reconcile(instance).await.unwrap();

        assert_eq!(
            outcome.requeue_after,
            Some(Duration::from_secs(STEADY_STATE_REQUEUE_SECONDS))
        );
    }

    #[tokio::test]
    async fn an_instance_that_is_not_ready_gets_no_theme() {
        let mut instance = ferriskey_with(Some(radius(6)), None);
        instance.status = None;
        let mut deployer = MockIdentityInstanceDeployer::new();
        deployer
            .expect_ensure_provider_resources()
            .returning(|_| Box::pin(async { Ok(()) }));
        deployer
            .expect_database_ready()
            .returning(|_| Box::pin(async { Ok(true) }));
        deployer
            .expect_provider_ready()
            .returning(|_| Box::pin(async { Ok(false) }));
        deployer
            .expect_edge_ready()
            .returning(|_| Box::pin(async { Ok(true) }));
        deployer
            .expect_upgrade_in_progress()
            .returning(|_| Box::pin(async { Ok(false) }));
        let mut repository = MockIdentityInstanceRepository::new();
        repository.expect_patch_status().returning(|instance, _| {
            let instance = instance.clone();
            Box::pin(async move { Ok(instance) })
        });

        let service = IdentityInstanceServiceImpl::new(
            Arc::new(repository),
            Arc::new(deployer),
            Arc::new(MockInstanceThemePort::new()),
        );
        service.reconcile(instance).await.unwrap();
    }
}
