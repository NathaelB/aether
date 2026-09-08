use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use aether_domain::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        ports::DataPlaneRepository,
        value_objects::{Capacity, DataPlaneId, DataPlaneMode, DataPlaneStatus, Region},
    },
};
use aether_macros::repository;
use aether_persistence::SharedTx;

#[derive(FromRow)]
struct DataPlaneRow {
    id: Uuid,
    mode: String,
    region: String,
    status: String,
    capacity: i32,
    last_seen_at: Option<DateTime<Utc>>,
}

impl DataPlaneRow {
    fn into_dataplane(self) -> Result<DataPlane, CoreError> {
        let mode = parse_mode(&self.mode)?;
        let status = parse_status(&self.status)?;
        let capacity = Capacity::new(self.capacity as u32)?;

        Ok(DataPlane {
            id: DataPlaneId(self.id),
            mode,
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
                   region,
                   status,
                   capacity,
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
                   region,
                   status,
                   capacity,
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
        region: Option<Region>,
        required_capacity: u32,
        seen_since: DateTime<Utc>,
    ) -> Result<Option<DataPlane>, CoreError> {
        let required_capacity = i64::from(required_capacity);
        let mut tx = self.tx.lock().await;

        // Two statements rather than one with an optional predicate: the
        // region filter changes the parameter positions, and sqlx checks each
        // query against the schema at compile time.
        let row = match region {
            Some(region) => {
                sqlx::query_as!(
                    DataPlaneRow,
                    r#"
                    SELECT dp.id,
                           dp.mode,
                           dp.region,
                           dp.status,
                           dp.capacity,
                           dp.last_seen_at
                    FROM data_planes dp
                    LEFT JOIN deployments d
                      ON d.dataplane_id = dp.id
                     AND d.deleted_at IS NULL
                    WHERE dp.region = $1
                      AND dp.mode = 'shared'
                      AND dp.status = 'active'
                      AND dp.last_seen_at >= $3
                    GROUP BY dp.id, dp.mode, dp.region, dp.status, dp.capacity, dp.last_seen_at
                    HAVING (dp.capacity::BIGINT - COUNT(d.id)) >= $2
                    ORDER BY COUNT(d.id) ASC
                    LIMIT 1
                    "#,
                    region.as_str(),
                    required_capacity,
                    seen_since
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
                           dp.region,
                           dp.status,
                           dp.capacity,
                           dp.last_seen_at
                    FROM data_planes dp
                    LEFT JOIN deployments d
                      ON d.dataplane_id = dp.id
                     AND d.deleted_at IS NULL
                    WHERE dp.mode = 'shared'
                      AND dp.status = 'active'
                      AND dp.last_seen_at >= $2
                    GROUP BY dp.id, dp.mode, dp.region, dp.status, dp.capacity, dp.last_seen_at
                    HAVING (dp.capacity::BIGINT - COUNT(d.id)) >= $1
                    ORDER BY COUNT(d.id) ASC
                    LIMIT 1
                    "#,
                    required_capacity,
                    seen_since
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
                   region,
                   status,
                   capacity,
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
                capacity,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id)
            DO UPDATE SET
                mode = $2,
                region = $3,
                status = $4,
                capacity = $5,
                updated_at = $7
            "#,
                dataplane.id.0,
                mode_to_string(dataplane.mode),
                dataplane.region.as_str(),
                status_to_string(dataplane.status),
                dataplane.capacity.max() as i32,
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
        let affected = sqlx::query!(
            r#"
            UPDATE data_planes
            SET last_seen_at = GREATEST(COALESCE(last_seen_at, $2), $2),
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
}

fn mode_to_string(mode: DataPlaneMode) -> &'static str {
    match mode {
        DataPlaneMode::Shared => "shared",
        DataPlaneMode::Dedicated => "dedicated",
    }
}

fn status_to_string(status: DataPlaneStatus) -> &'static str {
    match status {
        DataPlaneStatus::Active => "active",
        DataPlaneStatus::Draining => "draining",
        DataPlaneStatus::Disabled => "disabled",
    }
}

fn parse_mode(raw: &str) -> Result<DataPlaneMode, CoreError> {
    match raw.to_ascii_lowercase().as_str() {
        "shared" => Ok(DataPlaneMode::Shared),
        "dedicated" => Ok(DataPlaneMode::Dedicated),
        other => Err(CoreError::InternalError(format!(
            "Invalid data plane mode: {}",
            other
        ))),
    }
}

fn parse_status(raw: &str) -> Result<DataPlaneStatus, CoreError> {
    match raw.to_ascii_lowercase().as_str() {
        "active" => Ok(DataPlaneStatus::Active),
        "draining" => Ok(DataPlaneStatus::Draining),
        "disabled" => Ok(DataPlaneStatus::Disabled),
        other => Err(CoreError::InternalError(format!(
            "Invalid data plane status: {}",
            other
        ))),
    }
}
