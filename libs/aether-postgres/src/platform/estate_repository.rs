use aether_domain::{
    CoreError,
    dataplane::value_objects::{DataPlaneId, Region},
    deployments::{
        Deployment, DeploymentId, DeploymentKind, DeploymentName, DeploymentStatus,
        network::NetworkAccess,
    },
    organisation::{
        Organisation, OrganisationId,
        value_objects::{OrganisationLimits, OrganisationName, OrganisationSlug},
    },
    platform::{
        EstateDeployment, EstateOwner, EstatePage, EstateQuery, Tenant, TenantPage, TenantQuery,
        ports::EstateRepository,
    },
    user::UserId,
    version::Version,
};
use aether_macros::repository;
use aether_persistence::SharedTx;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow)]
struct EstateRow {
    id: Uuid,
    organisation_id: Uuid,
    organisation_name: String,
    dataplane_id: Uuid,
    region: String,
    name: String,
    kind: String,
    status: String,
    namespace: String,
    environment: String,
    offer: Option<String>,
    restored_from: Option<Uuid>,
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
}

impl EstateRow {
    fn into_estate_deployment(self) -> Result<EstateDeployment, CoreError> {
        let version = self
            .version
            .ok_or_else(|| {
                CoreError::InternalError(format!("deployment {} has no version", self.id))
            })
            .and_then(|raw| {
                Version::parse(&raw).map_err(|e| {
                    CoreError::InternalError(format!(
                        "deployment {} has version '{raw}': {e}",
                        self.id
                    ))
                })
            })?;

        Ok(EstateDeployment {
            organisation: EstateOwner {
                id: OrganisationId(self.organisation_id),
                name: self.organisation_name,
            },
            region: Region::new(self.region),
            deployment: Deployment {
                id: DeploymentId(self.id),
                organisation_id: OrganisationId(self.organisation_id),
                dataplane_id: DataPlaneId(self.dataplane_id),
                name: DeploymentName(self.name),
                kind: DeploymentKind::try_from(self.kind.as_str())?,
                version,
                status: DeploymentStatus::try_from(self.status.as_str())?,
                namespace: self.namespace,
                environment: self.environment.parse()?,
                offer: self.offer.as_deref().map(str::parse).transpose()?,
                restored_from: self.restored_from.map(aether_domain::backups::BackupId),
                resources: aether_domain::dataplane::value_objects::DeploymentResources::new(
                    self.cpu_millis as u32,
                    self.memory_mib as u32,
                    self.storage_gib as u32,
                )?,
                created_by: UserId(self.created_by),
                created_at: self.created_at,
                updated_at: self.updated_at,
                deployed_at: self.deployed_at,
                deleted_at: self.deleted_at,
                // The one parser, shared with the deployment repository: two
                // readings of the same column would disagree on the day one
                // of them gained a policy.
                auto_upgrade: crate::deployments::deployment_repository::parse_auto_upgrade(
                    &self.auto_upgrade,
                )?,
                // Neither travels in a list. The window is read on the
                // deployment's own page, and the ranges are a list of their own
                // that would dwarf every other field on a row.
                maintenance_window: None,
                network_access: NetworkAccess::Open,
            },
        })
    }
}

#[derive(FromRow)]
struct TenantRow {
    id: Uuid,
    name: String,
    slug: String,
    owner_id: Uuid,
    status: String,
    plan: String,
    max_instances: i32,
    max_users: i32,
    max_storage_gb: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
    deployments: i64,
    members: i64,
}

impl TenantRow {
    fn into_tenant(self) -> Result<Tenant, CoreError> {
        Ok(Tenant {
            deployments: self.deployments.max(0) as usize,
            members: self.members.max(0) as usize,
            organisation: Organisation {
                id: OrganisationId(self.id),
                name: OrganisationName::new(self.name)?,
                slug: OrganisationSlug::new(self.slug)?,
                owner_id: UserId(self.owner_id),
                status: self.status.parse()?,
                plan: self.plan.parse()?,
                limits: OrganisationLimits::custom(
                    self.max_instances as usize,
                    self.max_users as usize,
                    self.max_storage_gb as usize,
                ),
                created_at: self.created_at,
                updated_at: self.updated_at,
                deleted_at: self.deleted_at,
            },
        })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[repository(domain = Estate, backend = Postgres)]
pub struct PostgresEstateRepository<'tx> {
    tx: SharedTx<'tx>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'tx> PostgresEstateRepository<'tx> {
    pub fn new(tx: &SharedTx<'tx>) -> Self {
        Self { tx: tx.clone() }
    }
}

impl EstateRepository for PostgresEstateRepository<'_> {
    async fn list_deployments(&self, query: &EstateQuery) -> Result<EstatePage, CoreError> {
        // One more than asked for. A page exactly `limit` long is not evidence
        // that the estate ends there, and a caller left to guess asks once too
        // often for ever.
        let probe = query.limit as i64 + 1;

        let rows = {
            let mut tx = self.tx.lock().await;
            sqlx::query_as!(
                EstateRow,
                r#"
            SELECT d.id,
                   d.organisation_id,
                   o.name AS organisation_name,
                   d.dataplane_id,
                   dp.region,
                   d.name,
                   d.kind,
                   d.status,
                   d.namespace,
                   d.environment,
                   d.offer,
                   d.restored_from,
                   d.version,
                   d.cpu_millis,
                   d.memory_mib,
                   d.storage_gib,
                   d.created_by,
                   d.created_at,
                   d.updated_at,
                   d.deployed_at,
                   d.deleted_at,
                   d.auto_upgrade
            FROM deployments d
            JOIN organisations o ON o.id = d.organisation_id
            JOIN data_planes dp ON dp.id = d.dataplane_id
            WHERE d.status <> 'deleted'
              AND ($1::UUID IS NULL OR d.organisation_id = $1)
              AND ($2::UUID IS NULL OR d.dataplane_id = $2)
              AND ($3::TEXT IS NULL OR dp.region = $3)
              AND ($4::TEXT IS NULL OR d.status = $4)
              -- Keyset, against the row the cursor names. An offset would
              -- shift under a deployment created while somebody pages,
              -- showing one row twice and skipping another.
              AND ($5::UUID IS NULL OR (d.created_at, d.id) < (
                    SELECT c.created_at, c.id FROM deployments c WHERE c.id = $5
              ))
            ORDER BY d.created_at DESC, d.id DESC
            LIMIT $6
            "#,
                query.organisation.map(|organisation| organisation.0),
                query.dataplane.map(|dataplane| dataplane.0),
                query.region.as_ref().map(|region| region.as_str()),
                query.status.as_ref().map(|status| status.to_string()),
                query.cursor.map(|cursor| cursor.0),
                probe
            )
            .fetch_all(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list the estate: {e}"),
        })?;

        let more = rows.len() > query.limit;
        let deployments = rows
            .into_iter()
            .take(query.limit)
            .map(EstateRow::into_estate_deployment)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(EstatePage {
            next_cursor: more
                .then(|| deployments.last().map(|last| last.deployment.id))
                .flatten(),
            deployments,
        })
    }

    async fn find_tenant(
        &self,
        organisation_id: OrganisationId,
    ) -> Result<Option<Tenant>, CoreError> {
        let row = {
            let mut tx = self.tx.lock().await;
            sqlx::query_as!(
                TenantRow,
                r#"
            SELECT o.id,
                   o.name,
                   o.slug,
                   o.owner_id,
                   o.status,
                   o.plan,
                   o.max_instances,
                   o.max_users,
                   o.max_storage_gb,
                   o.created_at,
                   o.updated_at,
                   o.deleted_at,
                   -- The same two subqueries the listing uses, for the same
                   -- reason: joined, they multiply, and three deployments
                   -- beside two members would report six of each.
                   (SELECT COUNT(*) FROM deployments d
                     WHERE d.organisation_id = o.id
                       AND d.status <> 'deleted') AS "deployments!",
                   (SELECT COUNT(*) FROM members m
                     WHERE m.organisation_id = o.id) AS "members!"
            FROM organisations o
            WHERE o.id = $1
            "#,
                organisation_id.0
            )
            .fetch_optional(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to read the tenant: {e}"),
        })?;

        row.map(TenantRow::into_tenant).transpose()
    }

    async fn list_tenants(&self, query: &TenantQuery) -> Result<TenantPage, CoreError> {
        let probe = query.limit as i64 + 1;

        let rows = {
            let mut tx = self.tx.lock().await;
            sqlx::query_as!(
                TenantRow,
                r#"
            SELECT o.id,
                   o.name,
                   o.slug,
                   o.owner_id,
                   o.status,
                   o.plan,
                   o.max_instances,
                   o.max_users,
                   o.max_storage_gb,
                   o.created_at,
                   o.updated_at,
                   o.deleted_at,
                   -- Subqueries rather than two joins and a GROUP BY: joining
                   -- both would multiply the rows together, and an
                   -- organisation with three deployments and four members
                   -- would report twelve of each.
                   (SELECT COUNT(*) FROM deployments d
                     WHERE d.organisation_id = o.id
                       AND d.status <> 'deleted') AS "deployments!",
                   (SELECT COUNT(*) FROM members m
                     WHERE m.organisation_id = o.id) AS "members!"
            FROM organisations o
            WHERE ($1::TEXT IS NULL OR o.status = $1)
              AND ($2::UUID IS NULL OR (o.created_at, o.id) < (
                    SELECT c.created_at, c.id FROM organisations c WHERE c.id = $2
              ))
            ORDER BY o.created_at DESC, o.id DESC
            LIMIT $3
            "#,
                query.status.as_ref().map(|status| status.to_string()),
                query.cursor.map(|cursor| cursor.0),
                probe
            )
            .fetch_all(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list the tenants: {e}"),
        })?;

        let more = rows.len() > query.limit;
        let tenants = rows
            .into_iter()
            .take(query.limit)
            .map(TenantRow::into_tenant)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(TenantPage {
            next_cursor: more
                .then(|| tenants.last().map(|last| last.organisation.id))
                .flatten(),
            tenants,
        })
    }
}
