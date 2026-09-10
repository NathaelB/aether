use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use aether_domain::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        ports::DataPlaneRepository,
        value_objects::{
            Capacity, DataPlaneAllocation, DataPlaneId, DataPlaneMode, DataPlaneStatus,
            PlacementPolicy, PlacementRequest, Region,
        },
    },
    organisation::OrganisationId,
};
use aether_macros::repository;
use aether_persistence::SharedTx;

#[derive(FromRow)]
struct DataPlaneRow {
    id: Uuid,
    mode: String,
    organisation_id: Option<Uuid>,
    region: String,
    status: String,
    capacity_cpu_millis: i32,
    capacity_memory_mib: i32,
    capacity_storage_gib: i32,
    last_seen_at: Option<DateTime<Utc>>,
}

impl DataPlaneRow {
    fn into_dataplane(self) -> Result<DataPlane, CoreError> {
        let allocation = parse_allocation(&self.mode, self.organisation_id)?;
        let status = parse_status(&self.status)?;
        let capacity = Capacity::new(
            self.capacity_cpu_millis as u32,
            self.capacity_memory_mib as u32,
            self.capacity_storage_gib as u32,
        )?;

        Ok(DataPlane {
            id: DataPlaneId(self.id),
            allocation,
            region: Region::new(self.region),
            status,
            capacity,
            last_seen_at: self.last_seen_at,
        })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[repository(domain = DataPlane, backend = Postgres)]
pub struct PostgresDataPlaneRepository<'tx> {
    tx: SharedTx<'tx>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'tx> PostgresDataPlaneRepository<'tx> {
    pub fn new(tx: &SharedTx<'tx>) -> Self {
        Self { tx: tx.clone() }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl DataPlaneRepository for PostgresDataPlaneRepository<'_> {
    async fn find_by_id(&self, id: &DataPlaneId) -> Result<Option<DataPlane>, CoreError> {
        let row = {
            let mut tx = self.tx.lock().await;
            sqlx::query_as!(
                DataPlaneRow,
                r#"
            SELECT id,
                   mode,
                   organisation_id,
                   region,
                   status,
                   capacity_cpu_millis,
                   capacity_memory_mib,
                   capacity_storage_gib,
                   last_seen_at
            FROM data_planes
            WHERE id = $1
            "#,
                id.0
            )
            .fetch_optional(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to find data plane by id: {}", e),
        })?;

        row.map(|row| row.into_dataplane()).transpose()
    }

    async fn find_active_shared_by_region(
        &self,
        region: &Region,
    ) -> Result<Vec<DataPlane>, CoreError> {
        let rows = {
            let mut tx = self.tx.lock().await;
            sqlx::query_as!(
                DataPlaneRow,
                r#"
            SELECT id,
                   mode,
                   organisation_id,
                   region,
                   status,
                   capacity_cpu_millis,
                   capacity_memory_mib,
                   capacity_storage_gib,
                   last_seen_at
            FROM data_planes
            WHERE region = $1
              AND mode = 'shared'
              AND status = 'active'
            "#,
                region.as_str()
            )
            .fetch_all(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list active shared data planes: {}", e),
        })?;

        rows.into_iter().map(|row| row.into_dataplane()).collect()
    }

    async fn find_available(
        &self,
        request: PlacementRequest,
    ) -> Result<Option<DataPlane>, CoreError> {
        let mode = mode_to_row(request.mode);
        let cpu = i64::from(request.resources.cpu_millis);
        let memory = i64::from(request.resources.memory_mib);
        let storage = i64::from(request.resources.storage_gib);
        let organisation_id = request.organisation_id.0;

        // The ordering is the policy, expressed as a sign rather than as two
        // near-identical queries: least-used first spreads, most-used first
        // packs. sqlx checks the statement at compile time, so it cannot be
        // interpolated.
        let ordering = match request.policy {
            PlacementPolicy::Spread => 1_i64,
            PlacementPolicy::Pack => -1_i64,
        };

        let mut tx = self.tx.lock().await;

        // Two statements rather than one with an optional predicate: the
        // region filter changes the parameter positions, and sqlx checks each
        // query against the schema at compile time.
        let row = match request.region {
            Some(region) => {
                sqlx::query_as!(
                    DataPlaneRow,
                    r#"
                    SELECT dp.id,
                           dp.mode,
                           dp.organisation_id,
                           dp.region,
                           dp.status,
                           dp.capacity_cpu_millis,
                           dp.capacity_memory_mib,
                           dp.capacity_storage_gib,
                           dp.last_seen_at
                    FROM data_planes dp
                    LEFT JOIN deployments d
                      ON d.dataplane_id = dp.id
                     AND d.deleted_at IS NULL
                    WHERE dp.region = $1
                      AND dp.mode = $6
                      AND dp.status = 'active'
                      AND dp.last_seen_at >= $5
                      AND (dp.mode = 'shared' OR dp.organisation_id = $8)
                    GROUP BY dp.id, dp.mode, dp.organisation_id, dp.region, dp.status,
                             dp.capacity_cpu_millis, dp.capacity_memory_mib,
                             dp.capacity_storage_gib, dp.last_seen_at
                    HAVING dp.capacity_cpu_millis - COALESCE(SUM(d.cpu_millis), 0) >= $2
                       AND dp.capacity_memory_mib - COALESCE(SUM(d.memory_mib), 0) >= $3
                       AND dp.capacity_storage_gib - COALESCE(SUM(d.storage_gib), 0) >= $4
                    ORDER BY COALESCE(SUM(d.storage_gib), 0) * $7 ASC
                    LIMIT 1
                    "#,
                    region.as_str(),
                    cpu,
                    memory,
                    storage,
                    request.seen_since,
                    mode,
                    ordering,
                    organisation_id
                )
                .fetch_optional(&mut ***tx)
                .await
            }
            None => {
                sqlx::query_as!(
                    DataPlaneRow,
                    r#"
                    SELECT dp.id,
                           dp.mode,
                           dp.organisation_id,
                           dp.region,
                           dp.status,
                           dp.capacity_cpu_millis,
                           dp.capacity_memory_mib,
                           dp.capacity_storage_gib,
                           dp.last_seen_at
                    FROM data_planes dp
                    LEFT JOIN deployments d
                      ON d.dataplane_id = dp.id
                     AND d.deleted_at IS NULL
                    WHERE dp.mode = $5
                      AND dp.status = 'active'
                      AND dp.last_seen_at >= $4
                      AND (dp.mode = 'shared' OR dp.organisation_id = $7)
                    GROUP BY dp.id, dp.mode, dp.organisation_id, dp.region, dp.status,
                             dp.capacity_cpu_millis, dp.capacity_memory_mib,
                             dp.capacity_storage_gib, dp.last_seen_at
                    HAVING dp.capacity_cpu_millis - COALESCE(SUM(d.cpu_millis), 0) >= $1
                       AND dp.capacity_memory_mib - COALESCE(SUM(d.memory_mib), 0) >= $2
                       AND dp.capacity_storage_gib - COALESCE(SUM(d.storage_gib), 0) >= $3
                    ORDER BY COALESCE(SUM(d.storage_gib), 0) * $6 ASC
                    LIMIT 1
                    "#,
                    cpu,
                    memory,
                    storage,
                    request.seen_since,
                    mode,
                    ordering,
                    organisation_id
                )
                .fetch_optional(&mut ***tx)
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to find available data plane: {}", e),
        })?;

        row.map(|row| row.into_dataplane()).transpose()
    }

    async fn list_all(&self) -> Result<Vec<DataPlane>, CoreError> {
        let rows = {
            let mut tx = self.tx.lock().await;
            sqlx::query_as!(
                DataPlaneRow,
                r#"
            SELECT id,
                   mode,
                   organisation_id,
                   region,
                   status,
                   capacity_cpu_millis,
                   capacity_memory_mib,
                   capacity_storage_gib,
                   last_seen_at
            FROM data_planes
            ORDER BY region ASC, id ASC
            "#
            )
            .fetch_all(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list data planes: {}", e),
        })?;

        rows.into_iter().map(|row| row.into_dataplane()).collect()
    }

    async fn current_load(&self, id: &DataPlaneId) -> Result<u32, CoreError> {
        let count: i64 = {
            let mut tx = self.tx.lock().await;
            sqlx::query_scalar!(
                r#"
            SELECT COUNT(*)::BIGINT as "count!"
            FROM deployments
            WHERE dataplane_id = $1
              AND deleted_at IS NULL
            "#,
                id.0
            )
            .fetch_one(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to fetch data plane load: {}", e),
        })?;

        u32::try_from(count).map_err(|_| {
            CoreError::InternalError(format!("Invalid data plane load value: {}", count))
        })
    }

    async fn save(&self, dataplane: &DataPlane) -> Result<(), CoreError> {
        let now = Utc::now();
        {
            let mut tx = self.tx.lock().await;
            sqlx::query!(
                r#"
            INSERT INTO data_planes (
                id,
                mode,
                region,
                status,
                organisation_id,
                capacity_cpu_millis,
                capacity_memory_mib,
                capacity_storage_gib,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (id)
            DO UPDATE SET
                mode = $2,
                region = $3,
                status = $4,
                organisation_id = $5,
                capacity_cpu_millis = $6,
                capacity_memory_mib = $7,
                capacity_storage_gib = $8,
                updated_at = $10
            "#,
                dataplane.id.0,
                mode_to_string(dataplane.allocation.mode()),
                dataplane.region.as_str(),
                status_to_string(dataplane.status),
                dataplane.allocation.owner().map(|id| id.0),
                dataplane.capacity.cpu_millis() as i32,
                dataplane.capacity.memory_mib() as i32,
                dataplane.capacity.storage_gib() as i32,
                now,
                now,
            )
            .execute(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to save data plane: {}", e),
        })?;

        Ok(())
    }

    async fn touch_last_seen(
        &self,
        id: &DataPlaneId,
        at: DateTime<Utc>,
    ) -> Result<bool, CoreError> {
        let mut tx = self.tx.lock().await;

        // GREATEST, not a blind assignment: heartbeats can arrive out of order
        // when a data plane retries a request whose response was lost, and a
        // stale one must not move the timestamp backwards.
        //
        // The status promotion is in the same statement rather than a second
        // round trip, so a data plane can never be observed as reporting and
        // still provisioning.
        //
        // Only `provisioning` is promoted. `draining` and `disabled` are
        // decisions an operator made, and a cluster that keeps reporting while
        // being drained is exactly the expected behaviour -- promoting it would
        // undo the drain on the next heartbeat. `failed` stays put too: it
        // records that provisioning did not complete, and reviving it silently
        // would hide a half-built cluster.
        let affected = sqlx::query!(
            r#"
            UPDATE data_planes
            SET last_seen_at = GREATEST(COALESCE(last_seen_at, $2), $2),
                status = CASE WHEN status = 'provisioning' THEN 'active' ELSE status END,
                updated_at = $2
            WHERE id = $1
            "#,
            id.0,
            at
        )
        .execute(&mut ***tx)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to record data plane heartbeat: {}", e),
        })?
        .rows_affected();

        Ok(affected > 0)
    }

    async fn region_is_served(&self, region: &Region) -> Result<bool, CoreError> {
        let mut tx = self.tx.lock().await;

        // EXISTS, not a count: the question is whether the region is served at
        // all, and it is only asked once placement has already failed.
        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM data_planes WHERE region = $1
            ) AS "exists!"
            "#,
            region.as_str()
        )
        .fetch_one(&mut ***tx)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to check whether region is served: {}", e),
        })?;

        Ok(exists)
    }

    async fn find_dedicated_for_organisation(
        &self,
        organisation_id: &OrganisationId,
        region: &Region,
    ) -> Result<Option<DataPlane>, CoreError> {
        // Deliberately no status or last_seen_at filter, unlike
        // `find_available`: this runs *before* provisioning, to catch a
        // cluster that exists but has not reported yet.
        let row = {
            let mut tx = self.tx.lock().await;
            sqlx::query_as!(
                DataPlaneRow,
                r#"
            SELECT id,
                   mode,
                   organisation_id,
                   region,
                   status,
                   capacity_cpu_millis,
                   capacity_memory_mib,
                   capacity_storage_gib,
                   last_seen_at
            FROM data_planes
            WHERE region = $1
              AND mode = 'dedicated'
              AND organisation_id = $2
            "#,
                region.as_str(),
                organisation_id.0
            )
            .fetch_optional(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!(
                "Failed to find dedicated data plane for organisation: {}",
                e
            ),
        })?;

        row.map(|row| row.into_dataplane()).transpose()
    }
}

fn mode_to_string(mode: DataPlaneMode) -> &'static str {
    match mode {
        DataPlaneMode::Shared => "shared",
        DataPlaneMode::Dedicated => "dedicated",
    }
}

fn status_to_string(status: DataPlaneStatus) -> &'static str {
    match status {
        DataPlaneStatus::Provisioning => "provisioning",
        DataPlaneStatus::Active => "active",
        DataPlaneStatus::Draining => "draining",
        DataPlaneStatus::Disabled => "disabled",
        DataPlaneStatus::Failed => "failed",
    }
}

/// The inverse of `parse_mode`. Kept beside it so the two spellings cannot
/// drift apart.
fn mode_to_row(mode: DataPlaneMode) -> &'static str {
    match mode {
        DataPlaneMode::Shared => "shared",
        DataPlaneMode::Dedicated => "dedicated",
    }
}

/// Rebuilds the allocation from the two columns that carry it.
///
/// A dedicated row without an owner is rejected rather than defaulted. The
/// database has a CHECK preventing one, so reaching that arm means the schema
/// and the code disagree -- and guessing would put someone else's deployment
/// on a reserved cluster.
fn parse_allocation(
    raw: &str,
    organisation_id: Option<Uuid>,
) -> Result<DataPlaneAllocation, CoreError> {
    match (raw.to_ascii_lowercase().as_str(), organisation_id) {
        ("shared", None) => Ok(DataPlaneAllocation::Shared),
        ("shared", Some(_)) => Err(CoreError::InternalError(
            "shared data plane carries an organisation".to_string(),
        )),
        ("dedicated", Some(id)) => Ok(DataPlaneAllocation::Dedicated {
            organisation_id: OrganisationId(id),
        }),
        ("dedicated", None) => Err(CoreError::InternalError(
            "dedicated data plane has no organisation".to_string(),
        )),
        (other, _) => Err(CoreError::InternalError(format!(
            "Invalid data plane mode: {}",
            other
        ))),
    }
}

fn parse_status(raw: &str) -> Result<DataPlaneStatus, CoreError> {
    match raw.to_ascii_lowercase().as_str() {
        "provisioning" => Ok(DataPlaneStatus::Provisioning),
        "active" => Ok(DataPlaneStatus::Active),
        "draining" => Ok(DataPlaneStatus::Draining),
        "disabled" => Ok(DataPlaneStatus::Disabled),
        "failed" => Ok(DataPlaneStatus::Failed),
        other => Err(CoreError::InternalError(format!(
            "Invalid data plane status: {}",
            other
        ))),
    }
}
