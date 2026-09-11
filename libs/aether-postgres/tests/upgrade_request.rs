//! Requesting an upgrade, against the real repositories.
//!
//! The unit tests cover the rules with stubs. This covers the part stubs
//! cannot: that the catalogue lookup and the deployment write actually
//! cooperate, and that a refused upgrade leaves the row exactly as it was.
//!
//! Runs only when `DATABASE_URL` is set. See `heartbeat_activates.rs`.

use aether_domain::{
    CoreError,
    catalog::{
        BreakingRisk, Release, ReleaseId, ReleaseNotes, ReleaseStatus, ports::ReleaseRepository,
    },
    dataplane::{
        entities::DataPlane,
        ports::DataPlaneRepository,
        value_objects::{Capacity, DataPlaneAllocation, DeploymentResources, Region},
    },
    deployments::{
        Deployment, DeploymentId, DeploymentKind, DeploymentName, DeploymentStatus,
        ports::DeploymentRepository,
    },
    organisation::OrganisationId,
    upgrades::{
        commands::RequestUpgradeCommand, ports::UpgradeService, service::UpgradeServiceImpl,
    },
    user::UserId,
    version::{Version, VersionChange},
};
use aether_persistence::with_tx;
use aether_postgres::{
    catalog::PostgresReleaseRepository, dataplane::PostgresDataPlaneRepository,
    deployments::PostgresDeploymentRepository, upgrades::PostgresUpgradeRunRepository,
};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

mod support;
use support::pool;

/// This suite is about what the repositories do, not about who may ask. The
/// permission rule has its own tests beside the service, with a stub that can
/// refuse; wiring a real one here would only prove the stub works.
struct AlwaysAllowed;

impl aether_domain::upgrades::ports::UpgradePolicy for AlwaysAllowed {
    async fn can_upgrade_deployment(
        &self,
        _identity: aether_auth::Identity,
        _organisation_id: OrganisationId,
    ) -> Result<(), CoreError> {
        Ok(())
    }
}

fn caller() -> aether_auth::Identity {
    aether_auth::Identity::User(aether_auth::User {
        id: "tester".to_string(),
        username: "tester".to_string(),
        email: None,
        name: None,
        roles: vec![],
    })
}

/// Each test owns a major version and a region nobody else writes, so the
/// suite stays parallel against a shared database.
struct Fixture {
    tag: String,
    major: u64,
    organisation_id: OrganisationId,
    deployment_id: DeploymentId,
}

impl Fixture {
    fn new(major: u64) -> Self {
        Self {
            tag: format!("upgrade-{}", Uuid::new_v4()),
            major,
            organisation_id: OrganisationId(Uuid::new_v4()),
            deployment_id: DeploymentId(Uuid::new_v4()),
        }
    }

    fn version(&self, minor: u64, patch: u64) -> Version {
        Version::new(self.major, minor, patch)
    }
}

async fn clean(pool: &PgPool, fixture: &Fixture) {
    for statement in [
        "DELETE FROM deployments WHERE dataplane_id IN (SELECT id FROM data_planes WHERE region = $1)",
        "DELETE FROM data_planes WHERE region = $1",
        "DELETE FROM organisations WHERE name = $1",
        "DELETE FROM users WHERE name = $1",
    ] {
        sqlx::query(statement)
            .bind(&fixture.tag)
            .execute(pool)
            .await
            .expect("cleanup");
    }

    sqlx::query("DELETE FROM releases WHERE version LIKE $1")
        .bind(format!("{}.%", fixture.major))
        .execute(pool)
        .await
        .expect("cleanup");
}

/// Sets up an organisation, a data plane, a deployment at `current` in
/// `status`, and a catalogue holding `catalogue`. Returns what the service
/// answered and what the row looks like afterwards.
async fn request(
    pool: &PgPool,
    fixture: &Fixture,
    status: DeploymentStatus,
    current: Version,
    catalogue: Vec<(Version, ReleaseStatus)>,
    target: Version,
) -> (Result<VersionChange, CoreError>, DeploymentStatus, Version) {
    clean(pool, fixture).await;

    let result: Result<(Result<VersionChange, CoreError>, DeploymentStatus, Version), CoreError> =
        with_tx(
            pool,
            |e| CoreError::DatabaseError {
                message: e.to_string(),
            },
            async |tx| {
                let dataplanes = PostgresDataPlaneRepository::new(&tx);
                let deployments = PostgresDeploymentRepository::new(&tx);
                let releases = PostgresReleaseRepository::new(&tx);

                let user_id = UserId(Uuid::new_v4());
                {
                    let mut guard = tx.lock().await;
                    sqlx::query("INSERT INTO users (id, email, name, sub) VALUES ($1, $2, $3, $4)")
                        .bind(user_id.0)
                        .bind(format!("{}@upgrade.test", user_id.0))
                        .bind(&fixture.tag)
                        .bind(user_id.0.to_string())
                        .execute(&mut ***guard)
                        .await
                        .map_err(|e| CoreError::DatabaseError {
                            message: e.to_string(),
                        })?;

                    sqlx::query(
                        "INSERT INTO organisations \
                         (id, name, slug, owner_id, status, plan, max_instances, max_users, \
                          max_storage_gb, created_at, updated_at) \
                         VALUES ($1, $2, $3, $4, 'active', 'free', 5, 5, 5, now(), now())",
                    )
                    .bind(fixture.organisation_id.0)
                    .bind(&fixture.tag)
                    .bind(fixture.organisation_id.0.to_string())
                    .bind(user_id.0)
                    .execute(&mut ***guard)
                    .await
                    .map_err(|e| CoreError::DatabaseError {
                        message: e.to_string(),
                    })?;
                }

                let dataplane = DataPlane::new(
                    DataPlaneAllocation::Shared,
                    Region::new(&fixture.tag),
                    Capacity::new(8_000, 16_384, 200).expect("non-zero capacity"),
                );
                dataplanes.save(&dataplane).await?;

                let at = Utc::now();
                deployments
                    .insert(Deployment {
                        id: fixture.deployment_id,
                        organisation_id: fixture.organisation_id,
                        dataplane_id: dataplane.id,
                        name: DeploymentName("auth".to_string()),
                        kind: DeploymentKind::Ferriskey,
                        version: current,
                        status,
                        namespace: fixture.tag.clone(),
                        resources: DeploymentResources::DEFAULT,
                        created_by: user_id,
                        created_at: at,
                        updated_at: at,
                        deployed_at: None,
                        deleted_at: None,
                        auto_upgrade: Default::default(),
                        maintenance_window: None,
                    })
                    .await?;

                for (version, status) in catalogue {
                    let mut release = Release::announce(
                        ReleaseId::new(DeploymentKind::Ferriskey, version),
                        BreakingRisk::None,
                        ReleaseNotes("probe".to_string()),
                        at,
                    );
                    release.status = status;
                    releases.insert(release).await?;
                }

                let answer = UpgradeServiceImpl::new(
                    PostgresDeploymentRepository::new(&tx),
                    PostgresReleaseRepository::new(&tx),
                    PostgresUpgradeRunRepository::new(&tx),
                    AlwaysAllowed,
                )
                .request_upgrade(
                    caller(),
                    RequestUpgradeCommand {
                        organisation_id: fixture.organisation_id,
                        deployment_id: fixture.deployment_id,
                        target,
                    },
                )
                .await
                .map(|accepted| accepted.change);

                // Read back rather than trusting what the service returned:
                // the question is what the row says, not what the value in
                // hand says.
                let stored = deployments
                    .get_by_id(fixture.deployment_id)
                    .await?
                    .expect("the deployment is there");

                Ok((answer, stored.status, stored.version))
            },
        )
        .await;

    let answered = result.expect("the transaction committed");
    clean(pool, fixture).await;
    answered
}

#[tokio::test]
async fn an_accepted_upgrade_leaves_the_row_upgrading_on_the_old_version() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new(901);

    let (answer, status, version) = request(
        &pool,
        &fixture,
        DeploymentStatus::Successful,
        fixture.version(0, 0),
        vec![(fixture.version(1, 0), ReleaseStatus::Available)],
        fixture.version(1, 0),
    )
    .await;

    assert_eq!(answer.expect("accepted"), VersionChange::Minor);
    assert_eq!(status, DeploymentStatus::Upgrading);
    // The version moves when the data plane confirms it, not when the upgrade
    // is asked for. Writing it here would report a version that is not running.
    assert_eq!(version, fixture.version(0, 0));
}

#[tokio::test]
async fn a_busy_deployment_is_refused_and_left_alone() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new(902);

    let (answer, status, version) = request(
        &pool,
        &fixture,
        DeploymentStatus::Deleting,
        fixture.version(0, 0),
        vec![(fixture.version(1, 0), ReleaseStatus::Available)],
        fixture.version(1, 0),
    )
    .await;

    assert!(matches!(
        answer,
        Err(CoreError::DeploymentNotUpgradable { .. })
    ));
    assert_eq!(status, DeploymentStatus::Deleting, "the row was touched");
    assert_eq!(version, fixture.version(0, 0));
}

#[tokio::test]
async fn a_withdrawn_target_is_refused_and_the_row_is_left_alone() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new(903);

    let (answer, status, _) = request(
        &pool,
        &fixture,
        DeploymentStatus::Successful,
        fixture.version(0, 0),
        vec![(fixture.version(1, 0), ReleaseStatus::Withdrawn)],
        fixture.version(1, 0),
    )
    .await;

    assert!(matches!(
        answer,
        Err(CoreError::ReleaseNotInstallable { .. })
    ));
    assert_eq!(status, DeploymentStatus::Successful);
}

#[tokio::test]
async fn a_target_the_catalogue_does_not_hold_is_refused() {
    let Some(pool) = pool().await else {
        return;
    };
    let fixture = Fixture::new(904);

    let (answer, status, _) = request(
        &pool,
        &fixture,
        DeploymentStatus::Successful,
        fixture.version(0, 0),
        Vec::new(),
        fixture.version(1, 0),
    )
    .await;

    assert!(matches!(answer, Err(CoreError::ReleaseNotFound { .. })));
    assert_eq!(status, DeploymentStatus::Successful);
}
