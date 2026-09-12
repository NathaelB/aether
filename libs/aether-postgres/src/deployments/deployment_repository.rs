use aether_domain::dataplane::value_objects::DeploymentResources;
use chrono::{DateTime, Utc, Weekday};
use sqlx::FromRow;
use uuid::Uuid;

use aether_domain::{
    CoreError,
    dataplane::value_objects::DataPlaneId,
    deployments::{
        Deployment, DeploymentId, DeploymentKind, DeploymentName, DeploymentStatus,
        network::{Cidr, NetworkAccess},
        ports::DeploymentRepository,
    },
    organisation::OrganisationId,
    upgrades::policy::{AutoUpgradePolicy, MaintenanceWindow},
    user::UserId,
    version::Version,
};
use aether_macros::repository;
use aether_persistence::SharedTx;

#[derive(FromRow)]
struct DeploymentRow {
    id: Uuid,
    organisation_id: Uuid,
    dataplane_id: Uuid,
    name: String,
    kind: String,
    status: String,
    namespace: String,
    version: Option<String>,
    cpu_millis: i32,
    memory_mib: i32,
    storage_gib: i32,
    created_by: Uuid,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deployed_at: Option<DateTime<Utc>>,
    deleted_at: Option<DateTime<Utc>>,
    auto_upgrade: String,
    maintenance_day: Option<String>,
    maintenance_start: Option<chrono::NaiveTime>,
    maintenance_minutes: Option<i32>,
    maintenance_timezone: Option<String>,
    /// NULL is open. Read as text because the column is CIDR[]: sqlx maps
    /// that only with a network-types feature the workspace does not carry,
    /// and the domain parses the string anyway.
    allowed_cidrs: Option<Vec<String>>,
}

impl DeploymentRow {
    fn into_deployment(self) -> Result<Deployment, CoreError> {
        let kind = DeploymentKind::try_from(self.kind.as_str())?;
        let status = DeploymentStatus::try_from(self.status.as_str())?;
        // Parsed rather than wrapped: the column is NOT NULL and checked
        // since the semver migration, so a row that fails here means the
        // schema and the domain have drifted apart, and saying so beats
        // carrying an unusable version further in.
        let raw = self.version.ok_or_else(|| {
            CoreError::InternalError(format!("deployment {} has no version", self.id))
        })?;
        let version = Version::parse(&raw).map_err(|e| {
            CoreError::InternalError(format!("deployment {} has version '{raw}': {e}", self.id))
        })?;

        // All four columns or none, which the schema enforces. Reading them
        // one at a time and tolerating a partial row would put the question
        // back at every call site.
        let maintenance_window = match (
            self.maintenance_day,
            self.maintenance_start,
            self.maintenance_minutes,
            self.maintenance_timezone,
        ) {
            (Some(day), Some(start), Some(minutes), Some(zone)) => {
                Some(parse_window(&day, start, minutes, &zone)?)
            }
            _ => None,
        };

        Ok(Deployment {
            id: DeploymentId(self.id),
            organisation_id: OrganisationId(self.organisation_id),
            dataplane_id: DataPlaneId(self.dataplane_id),
            name: DeploymentName(self.name),
            kind,
            version,
            status,
            namespace: self.namespace,
            resources: DeploymentResources::new(
                self.cpu_millis as u32,
                self.memory_mib as u32,
                self.storage_gib as u32,
            )?,
            created_by: UserId(self.created_by),
            created_at: self.created_at,
            updated_at: self.updated_at,
            deployed_at: self.deployed_at,
            deleted_at: self.deleted_at,
            auto_upgrade: parse_auto_upgrade(&self.auto_upgrade)?,
            maintenance_window,
            network_access: parse_network_access(self.allowed_cidrs, self.id)?,
        })
    }
}

/// NULL is open; anything else has to parse.
///
/// A row that fails here means the column and the domain have drifted apart,
/// which is worth saying rather than carrying an unusable rule further in --
/// the same call the version column already makes one field up.
fn parse_network_access(
    allowed: Option<Vec<String>>,
    deployment: Uuid,
) -> Result<NetworkAccess, CoreError> {
    let Some(allowed) = allowed else {
        return Ok(NetworkAccess::Open);
    };

    let ranges = allowed
        .iter()
        .map(|raw| raw.parse::<Cidr>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            CoreError::InternalError(format!(
                "deployment {deployment} has an unusable range: {e}"
            ))
        })?;

    NetworkAccess::from_ranges(ranges).map_err(|e| {
        CoreError::InternalError(format!(
            "deployment {deployment} has an unusable allow list: {e}"
        ))
    })
}

/// The rows to write, where open is no rows at all.
///
/// `None` rather than an empty array: the column refuses an empty one, and
/// the two states have to stay one apart in the database exactly as they are
/// in the type.
fn network_access_to_row(access: &NetworkAccess) -> Option<Vec<String>> {
    match access {
        NetworkAccess::Open => None,
        NetworkAccess::Restricted { allowed } => {
            Some(allowed.ranges().iter().map(ToString::to_string).collect())
        }
    }
}

fn auto_upgrade_to_row(policy: AutoUpgradePolicy) -> &'static str {
    match policy {
        AutoUpgradePolicy::Manual => "manual",
        AutoUpgradePolicy::Patch => "patch",
        AutoUpgradePolicy::PatchAndMinor => "patch_and_minor",
    }
}

fn parse_auto_upgrade(value: &str) -> Result<AutoUpgradePolicy, CoreError> {
    match value {
        "manual" => Ok(AutoUpgradePolicy::Manual),
        "patch" => Ok(AutoUpgradePolicy::Patch),
        "patch_and_minor" => Ok(AutoUpgradePolicy::PatchAndMinor),
        other => Err(CoreError::InternalError(format!(
            "unknown auto upgrade policy '{other}'"
        ))),
    }
}

fn weekday_to_row(day: Weekday) -> &'static str {
    match day {
        Weekday::Mon => "mon",
        Weekday::Tue => "tue",
        Weekday::Wed => "wed",
        Weekday::Thu => "thu",
        Weekday::Fri => "fri",
        Weekday::Sat => "sat",
        Weekday::Sun => "sun",
    }
}

fn parse_window(
    day: &str,
    start: chrono::NaiveTime,
    minutes: i32,
    zone: &str,
) -> Result<MaintenanceWindow, CoreError> {
    let day = match day {
        "mon" => Weekday::Mon,
        "tue" => Weekday::Tue,
        "wed" => Weekday::Wed,
        "thu" => Weekday::Thu,
        "fri" => Weekday::Fri,
        "sat" => Weekday::Sat,
        "sun" => Weekday::Sun,
        other => {
            return Err(CoreError::InternalError(format!(
                "unknown weekday '{other}'"
            )));
        }
    };

    let timezone: chrono_tz::Tz = zone
        .parse()
        .map_err(|_| CoreError::InternalError(format!("unknown time zone '{zone}'")))?;

    MaintenanceWindow::new(
        day,
        start,
        chrono::Duration::minutes(minutes.into()),
        timezone,
    )
    .map_err(|e| CoreError::InternalError(format!("stored maintenance window is invalid: {e}")))
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[repository(domain = Deployment, backend = Postgres)]
pub struct PostgresDeploymentRepository<'tx> {
    tx: SharedTx<'tx>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'tx> PostgresDeploymentRepository<'tx> {
    pub fn new(tx: &SharedTx<'tx>) -> Self {
        Self { tx: tx.clone() }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl DeploymentRepository for PostgresDeploymentRepository<'_> {
    async fn purge_deleted(&self, before: DateTime<Utc>) -> Result<u64, CoreError> {
        let affected = {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            DELETE FROM deployments
            WHERE status = 'deleted'
              AND updated_at < $1
            "#,
                before
            )
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to purge deleted deployments: {}", e),
        })?
        .rows_affected();

        Ok(affected)
    }

    async fn insert(&self, deployment: Deployment) -> Result<(), CoreError> {
        // Bound before the query rather than inline: the slice is borrowed for
        // the whole call, and a temporary built in the argument list is gone
        // before it is read.
        let allowed_cidrs = network_access_to_row(&deployment.network_access);
        {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            INSERT INTO deployments (
                id,
                organisation_id,
                dataplane_id,
                name,
                kind,
                status,
                namespace,
                version,
                cpu_millis,
                memory_mib,
                storage_gib,
                created_by,
                created_at,
                updated_at,
                deployed_at,
                deleted_at,
                auto_upgrade,
                maintenance_day,
                maintenance_start,
                maintenance_minutes,
                maintenance_timezone,
                allowed_cidrs
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16,
                    $17, $18, $19, $20, $21, $22::TEXT[]::CIDR[])
            "#,
                deployment.id.0,
                deployment.organisation_id.0,
                deployment.dataplane_id.0,
                deployment.name.0,
                deployment.kind.to_string(),
                deployment.status.to_string(),
                deployment.namespace,
                deployment.version.to_string(),
                deployment.resources.cpu_millis as i32,
                deployment.resources.memory_mib as i32,
                deployment.resources.storage_gib as i32,
                deployment.created_by.0,
                deployment.created_at,
                deployment.updated_at,
                deployment.deployed_at,
                deployment.deleted_at,
                auto_upgrade_to_row(deployment.auto_upgrade),
                deployment
                    .maintenance_window
                    .as_ref()
                    .map(|w| weekday_to_row(w.day)),
                deployment.maintenance_window.as_ref().map(|w| w.start),
                deployment
                    .maintenance_window
                    .as_ref()
                    .map(|w| w.duration.num_minutes() as i32),
                deployment
                    .maintenance_window
                    .as_ref()
                    .map(|w| w.timezone.name().to_string()),
                allowed_cidrs.as_deref(),
            )
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to insert deployment: {}", e),
        })?;

        Ok(())
    }

    async fn get_by_id(
        &self,
        deployment_id: DeploymentId,
    ) -> Result<Option<Deployment>, CoreError> {
        let row = {
            let mut tx = self.tx.lock().await;
            sqlx::query_as!(
                DeploymentRow,
                r#"
            SELECT id,
                   organisation_id,
                   dataplane_id,
                   name,
                   kind,
                   status,
                   namespace,
                   cpu_millis,
                   memory_mib,
                   storage_gib,
                   version,
                   created_by,
                   created_at,
                   updated_at,
                   deployed_at,
                   deleted_at,
                   auto_upgrade,
                   maintenance_day,
                   maintenance_start,
                   maintenance_minutes,
                   maintenance_timezone,
                   allowed_cidrs::TEXT[] AS "allowed_cidrs: Vec<String>"
            FROM deployments
            WHERE id = $1
            "#,
                deployment_id.0
            )
            .fetch_optional(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to get deployment by id: {}", e),
        })?;

        row.map(|r| r.into_deployment()).transpose()
    }

    async fn list_by_organisation(
        &self,
        organisation_id: OrganisationId,
    ) -> Result<Vec<Deployment>, CoreError> {
        let rows = {
            let mut tx = self.tx.lock().await;
            sqlx::query_as!(
                DeploymentRow,
                r#"
            SELECT id,
                   organisation_id,
                   dataplane_id,
                   name,
                   kind,
                   status,
                   namespace,
                   cpu_millis,
                   memory_mib,
                   storage_gib,
                   version,
                   created_by,
                   created_at,
                   updated_at,
                   deployed_at,
                   deleted_at,
                   auto_upgrade,
                   maintenance_day,
                   maintenance_start,
                   maintenance_minutes,
                   maintenance_timezone,
                   allowed_cidrs::TEXT[] AS "allowed_cidrs: Vec<String>"
            FROM deployments
            WHERE organisation_id = $1
              AND status <> 'deleted'
            ORDER BY created_at DESC
            "#,
                organisation_id.0
            )
            .fetch_all(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list deployments by organisation: {}", e),
        })?;

        rows.into_iter().map(|r| r.into_deployment()).collect()
    }

    async fn update(&self, deployment: Deployment) -> Result<(), CoreError> {
        let allowed_cidrs = network_access_to_row(&deployment.network_access);
        {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            -- Every column a Deployment can change, because that is what this
            -- claims to write. It used to stop at deleted_at, so setting an
            -- upgrade policy or a maintenance window returned the new
            -- deployment to its caller and left the row exactly as it was.
            UPDATE deployments
            SET name = $2,
                kind = $3,
                status = $4,
                namespace = $5,
                version = $6,
                updated_at = $7,
                deployed_at = $8,
                deleted_at = $9,
                auto_upgrade = $10,
                maintenance_day = $11,
                maintenance_start = $12,
                maintenance_minutes = $13,
                maintenance_timezone = $14,
                allowed_cidrs = $15::TEXT[]::CIDR[]
            WHERE id = $1
            "#,
                deployment.id.0,
                deployment.name.0,
                deployment.kind.to_string(),
                deployment.status.to_string(),
                deployment.namespace,
                deployment.version.to_string(),
                deployment.updated_at,
                deployment.deployed_at,
                deployment.deleted_at,
                auto_upgrade_to_row(deployment.auto_upgrade),
                deployment
                    .maintenance_window
                    .as_ref()
                    .map(|w| weekday_to_row(w.day)),
                deployment.maintenance_window.as_ref().map(|w| w.start),
                deployment
                    .maintenance_window
                    .as_ref()
                    .map(|w| w.duration.num_minutes() as i32),
                deployment
                    .maintenance_window
                    .as_ref()
                    .map(|w| w.timezone.name().to_string()),
                allowed_cidrs.as_deref(),
            )
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to update deployment: {}", e),
        })?;

        Ok(())
    }

    async fn delete(&self, deployment_id: DeploymentId) -> Result<(), CoreError> {
        {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            UPDATE deployments
            SET deleted_at = $2,
                updated_at = $2,
                status = 'deleting'
            WHERE id = $1
            "#,
                deployment_id.0,
                Utc::now()
            )
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to delete deployment: {}", e),
        })?;

        Ok(())
    }

    async fn count_by_version(
        &self,
        kind: &DeploymentKind,
    ) -> Result<Vec<(Version, u64)>, CoreError> {
        let kind = kind.to_string();

        let rows = {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            SELECT version, COUNT(*) AS "count!"
            FROM deployments
            WHERE kind = $1
              AND status NOT IN ('deleting', 'deleted')
            GROUP BY version
            "#,
                kind
            )
            .fetch_all(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to count deployments by version: {e}"),
        })?;

        rows.into_iter()
            .map(|row| {
                let version = Version::parse(&row.version).map_err(|e| {
                    CoreError::InternalError(format!(
                        "a deployment holds version '{}': {e}",
                        row.version
                    ))
                })?;
                Ok((version, row.count as u64))
            })
            .collect()
    }

    async fn list_by_dataplane(
        &self,
        dataplane_id: &DataPlaneId,
    ) -> Result<Vec<Deployment>, CoreError> {
        {
            let mut tx = self.tx.lock().await;
            let rows = sqlx::query_as!(
                DeploymentRow,
                r#"
            SELECT id,
                   organisation_id,
                   dataplane_id,
                   name,
                   kind,
                   status,
                   namespace,
                   cpu_millis,
                   memory_mib,
                   storage_gib,
                   version,
                   created_by,
                   created_at,
                   updated_at,
                   deployed_at,
                   deleted_at,
                   auto_upgrade,
                   maintenance_day,
                   maintenance_start,
                   maintenance_minutes,
                   maintenance_timezone,
                   allowed_cidrs::TEXT[] AS "allowed_cidrs: Vec<String>"
            FROM deployments
            WHERE dataplane_id = $1
            ORDER BY created_at DESC
            "#,
                dataplane_id.0
            )
            .fetch_all(&mut ***tx)
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: format!("Failed to list deployments by dataplane: {}", e),
            })?;

            rows.into_iter().map(|r| r.into_deployment()).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn sample_time() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn sample_row() -> DeploymentRow {
        DeploymentRow {
            id: Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap(),
            organisation_id: Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap(),
            dataplane_id: Uuid::parse_str("dddddddd-dddd-dddd-dddd-dddddddddddd").unwrap(),
            name: "alpha".to_string(),
            cpu_millis: 500,
            memory_mib: 1024,
            storage_gib: 1,
            kind: "ferriskey".to_string(),
            status: "successful".to_string(),
            namespace: "ns-alpha".to_string(),
            version: Some("1.2.3".to_string()),
            created_by: Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap(),
            created_at: sample_time(),
            updated_at: sample_time(),
            deployed_at: Some(sample_time()),
            deleted_at: None,
            auto_upgrade: "manual".to_string(),
            maintenance_day: None,
            maintenance_start: None,
            maintenance_minutes: None,
            maintenance_timezone: None,
            allowed_cidrs: None,
        }
    }

    #[test]
    fn deployment_row_into_deployment_maps_fields() {
        let row = sample_row();
        let deployment = row.into_deployment().unwrap();

        assert_eq!(
            deployment.id.0,
            Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap()
        );
        assert_eq!(
            deployment.organisation_id.0,
            Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap()
        );
        assert_eq!(
            deployment.dataplane_id.0,
            Uuid::parse_str("dddddddd-dddd-dddd-dddd-dddddddddddd").unwrap()
        );
        assert_eq!(deployment.name.0, "alpha");
        assert_eq!(deployment.kind, DeploymentKind::Ferriskey);
        assert_eq!(deployment.status, DeploymentStatus::Successful);
        assert_eq!(deployment.namespace, "ns-alpha");
        assert_eq!(deployment.version.to_string(), "1.2.3");
        assert_eq!(
            deployment.created_by.0,
            Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap()
        );
        assert_eq!(deployment.created_at, sample_time());
        assert_eq!(deployment.updated_at, sample_time());
        assert_eq!(deployment.deployed_at, Some(sample_time()));
        assert!(deployment.deleted_at.is_none());
    }

    #[test]
    fn deployment_row_into_deployment_rejects_invalid_kind() {
        let mut row = sample_row();
        row.kind = "bogus".to_string();

        let err = row.into_deployment().unwrap_err();
        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("Invalid deployment kind"))
        );
    }

    #[test]
    fn deployment_row_into_deployment_rejects_invalid_status() {
        let mut row = sample_row();
        row.status = "unknown".to_string();

        let err = row.into_deployment().unwrap_err();
        assert!(
            matches!(err, CoreError::InternalError(message) if message.contains("Invalid deployment status"))
        );
    }
}
