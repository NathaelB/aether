use aether_domain::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        ports::DataPlaneRepository,
        value_objects::{Capacity, DataPlaneAllocation, DataPlaneId, DeploymentResources, Region},
    },
    deployments::{
        Deployment, DeploymentId, DeploymentKind, DeploymentName, DeploymentStatus,
        network::NetworkAccess, ports::DeploymentRepository,
    },
    iam_settings::{
        IamSettings,
        branding::{Branding, BrandingInput},
    },
    organisation::OrganisationId,
    upgrades::policy::AutoUpgradePolicy,
    user::UserId,
    version::Version,
};
use aether_persistence::in_scratch_tx;
use aether_postgres::{
    dataplane::PostgresDataPlaneRepository, deployments::PostgresDeploymentRepository,
};
use chrono::Utc;
use uuid::Uuid;

mod support;
use support::pool;

const TEST_REGION: &str = "iam-settings-test";

fn db(e: sqlx::Error) -> CoreError {
    CoreError::DatabaseError {
        message: e.to_string(),
    }
}

fn branding() -> Branding {
    let mut input = BrandingInput::default();
    input.colors.primary = Some("#112233".to_string());
    input.colors.error = Some("#FF0000".to_string());
    input.radius = Some(8);
    Branding::try_from(input).expect("valid")
}

fn settings() -> IamSettings {
    IamSettings {
        branding: Some(branding()),
    }
}

async fn stored(
    tx: &aether_persistence::SharedTx<'_>,
    id: DeploymentId,
) -> Result<Option<serde_json::Value>, CoreError> {
    let mut guard = tx.lock().await;
    sqlx::query_scalar("SELECT iam_settings FROM deployments WHERE id = $1")
        .bind(id.0)
        .fetch_one(&mut ***guard)
        .await
        .map_err(db)
}

#[tokio::test]
async fn the_migration_adds_a_nullable_jsonb_column() {
    let Some(pool) = pool().await else {
        return;
    };

    let column: (String, String) = sqlx::query_as(
        "SELECT data_type, is_nullable FROM information_schema.columns \
         WHERE table_name = 'deployments' AND column_name = 'iam_settings'",
    )
    .fetch_one(&pool)
    .await
    .expect("the column exists");

    assert_eq!(column, ("jsonb".to_string(), "YES".to_string()));
}

#[tokio::test]
async fn a_new_deployment_has_no_settings_and_stores_null() {
    let Some(pool) = pool().await else {
        return;
    };

    let read: Result<(IamSettings, Option<serde_json::Value>), CoreError> =
        in_scratch_tx(&pool, db, async |tx| {
            let deployments = PostgresDeploymentRepository::new(&tx);
            let deployment = seed(&tx).await?;
            let back = deployments.get_by_id(deployment.id).await?.expect("it");
            Ok((back.iam_settings, stored(&tx, deployment.id).await?))
        })
        .await;

    clean_up(&pool).await;

    let (settings, column) = read.expect("the transaction");
    assert_eq!(settings, IamSettings::default());
    assert_eq!(column, None);
}

#[tokio::test]
async fn branding_survives_the_round_trip_and_null_clears_it() {
    let Some(pool) = pool().await else {
        return;
    };

    let read: Result<(IamSettings, IamSettings, Option<serde_json::Value>), CoreError> =
        in_scratch_tx(&pool, db, async |tx| {
            let deployments = PostgresDeploymentRepository::new(&tx);
            let mut deployment = seed(&tx).await?;

            deployment.iam_settings = settings();
            deployments.update(deployment.clone()).await?;
            let with = deployments.get_by_id(deployment.id).await?.expect("it");

            deployment.iam_settings = IamSettings::default();
            deployments.update(deployment.clone()).await?;
            let without = deployments.get_by_id(deployment.id).await?.expect("it");

            Ok((
                with.iam_settings,
                without.iam_settings,
                stored(&tx, deployment.id).await?,
            ))
        })
        .await;

    clean_up(&pool).await;

    let (with, without, column) = read.expect("the transaction");
    assert_eq!(with, settings());
    assert_eq!(without, IamSettings::default());
    assert_eq!(column, None);
}

#[tokio::test]
async fn settings_given_at_insert_are_stored_as_the_wire_shape() {
    let Some(pool) = pool().await else {
        return;
    };

    let read: Result<(IamSettings, Option<serde_json::Value>), CoreError> =
        in_scratch_tx(&pool, db, async |tx| {
            let deployments = PostgresDeploymentRepository::new(&tx);
            let mut deployment = seed(&tx).await?;
            deployment.id = DeploymentId(Uuid::new_v4());
            deployment.name = DeploymentName("iamsettings-two".to_string());
            deployment.iam_settings = settings();
            deployments.insert(deployment.clone()).await?;

            let back = deployments.get_by_id(deployment.id).await?.expect("it");
            Ok((back.iam_settings, stored(&tx, deployment.id).await?))
        })
        .await;

    clean_up(&pool).await;

    let (settings_back, column) = read.expect("the transaction");
    assert_eq!(settings_back, settings());
    assert_eq!(
        column.expect("written"),
        serde_json::json!({
            "branding": {
                "colors": { "primary": "#112233", "error": "#FF0000" },
                "radius": 8
            }
        })
    );
}

#[tokio::test]
async fn a_stored_value_that_no_longer_parses_is_an_error_not_silence() {
    let Some(pool) = pool().await else {
        return;
    };

    let read: Result<(), CoreError> = in_scratch_tx(&pool, db, async |tx| {
        let deployments = PostgresDeploymentRepository::new(&tx);
        let deployment = seed(&tx).await?;

        for garbage in [
            r#"{"branding": {"radius": 99}}"#,
            r#"{"branding": {"colors": {"primary": "red"}}}"#,
            r#"{"branding": {"font": "serif"}}"#,
            r#"[1, 2]"#,
        ] {
            let mut guard = tx.lock().await;
            sqlx::query("UPDATE deployments SET iam_settings = $2::jsonb WHERE id = $1")
                .bind(deployment.id.0)
                .bind(garbage)
                .execute(&mut ***guard)
                .await
                .map_err(db)?;
            drop(guard);

            let refused = deployments.get_by_id(deployment.id).await;
            assert!(
                matches!(&refused, Err(CoreError::InternalError(message)) if message.contains("IAM settings")),
                "{garbage} was read as {refused:?}"
            );
        }

        Ok(())
    })
    .await;

    clean_up(&pool).await;

    read.expect("the transaction");
}

async fn clean_up(pool: &sqlx::PgPool) {
    for statement in [
        "DELETE FROM actions WHERE deployment_id IN (SELECT id FROM deployments WHERE dataplane_id IN (SELECT id FROM data_planes WHERE region = $1))",
        "DELETE FROM deployments WHERE dataplane_id IN (SELECT id FROM data_planes WHERE region = $1)",
        "DELETE FROM data_planes WHERE region = $1",
        "DELETE FROM organisations WHERE name = $1",
        "DELETE FROM users WHERE name = 'iamsettings'",
    ] {
        sqlx::query(statement)
            .bind(TEST_REGION)
            .execute(pool)
            .await
            .expect("cleanup");
    }
}

async fn seed(tx: &aether_persistence::SharedTx<'_>) -> Result<Deployment, CoreError> {
    let dataplanes = PostgresDataPlaneRepository::new(tx);
    let deployments = PostgresDeploymentRepository::new(tx);

    let dataplane = DataPlane::new(
        DataPlaneAllocation::Shared,
        Region::new(TEST_REGION),
        Capacity::new(8_000, 16_384, 200).expect("non-zero capacity"),
    );
    dataplanes.save(&dataplane).await?;

    let organisation_id = OrganisationId(Uuid::new_v4());
    let user_id = UserId(Uuid::new_v4());
    {
        let mut guard = tx.lock().await;
        sqlx::query("INSERT INTO users (id, email, name, sub) VALUES ($1, $2, 'iamsettings', $3)")
            .bind(user_id.0)
            .bind(format!("{}@iamsettings.test", user_id.0))
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
        .bind(organisation_id.0)
        .bind(TEST_REGION)
        .bind(organisation_id.0.to_string())
        .bind(user_id.0)
        .execute(&mut ***guard)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: e.to_string(),
        })?;
    }

    let now = Utc::now();
    let deployment = Deployment {
        id: DeploymentId(Uuid::new_v4()),
        organisation_id,
        dataplane_id: DataPlaneId(dataplane.id.0),
        name: DeploymentName("iamsettings".to_string()),
        kind: DeploymentKind::Ferriskey,
        version: Version::new(0, 5, 0),
        status: DeploymentStatus::Successful,
        namespace: "iamsettings-test".to_string(),
        environment: aether_domain::deployments::environment::Environment::Development,
        offer: None,
        restored_from: None,
        resources: DeploymentResources::DEFAULT,
        created_by: user_id,
        created_at: now,
        updated_at: now,
        deployed_at: None,
        deleted_at: None,
        auto_upgrade: AutoUpgradePolicy::Manual,
        maintenance_window: None,
        network_access: NetworkAccess::Open,
        last_verified_restore_at: None,
        last_restore_drill_seconds: None,
        log_shipping_enabled: false,
        iam_settings: Default::default(),
    };
    deployments.insert(deployment.clone()).await?;

    Ok(deployment)
}
