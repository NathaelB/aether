use chrono::Utc;
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
use aether_persistence::{PgExecutor, PgTransaction};

#[derive(FromRow)]
struct DataPlaneRow {
    id: Uuid,
    mode: String,
    region: String,
    status: String,
    capacity: i32,
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
        })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub struct PostgresDataPlaneRepository<'e, 't> {
    executor: PgExecutor<'e, 't>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'e, 't> PostgresDataPlaneRepository<'e, 't> {
    pub fn new(executor: PgExecutor<'e, 't>) -> Self {
        Self { executor }
    }

    pub fn from_tx(tx: &'e PgTransaction<'t>) -> Self {
        Self::new(PgExecutor::from_tx(tx))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'e> PostgresDataPlaneRepository<'e, 'e> {
    pub fn from_pool(pool: &'e sqlx::PgPool) -> Self {
        Self::new(PgExecutor::from_pool(pool))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl DataPlaneRepository for PostgresDataPlaneRepository<'_, '_> {
    async fn find_by_id(&self, id: &DataPlaneId) -> Result<Option<DataPlane>, CoreError> {
        let row = match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query_as!(
                    DataPlaneRow,
                    r#"
                    SELECT id,
                           mode,
                           region,
                           status,
                           capacity
                    FROM data_planes
                    WHERE id = $1
                    "#,
                    id.0
                )
                .fetch_optional(*pool)
                .await
            }
            PgExecutor::Tx(tx) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query_as!(
                    DataPlaneRow,
                    r#"
                    SELECT id,
                           mode,
                           region,
                           status,
                           capacity
                    FROM data_planes
                    WHERE id = $1
                    "#,
                    id.0
                )
                .fetch_optional(transaction.as_mut())
                .await
            }
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
        let rows = match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query_as!(
                    DataPlaneRow,
                    r#"
                    SELECT id,
                           mode,
                           region,
                           status,
                           capacity
                    FROM data_planes
                    WHERE region = $1
                      AND mode = 'shared'
                      AND status = 'active'
                    "#,
                    region.as_str()
                )
                .fetch_all(*pool)
                .await
            }
            PgExecutor::Tx(tx) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query_as!(
                    DataPlaneRow,
                    r#"
                    SELECT id,
                           mode,
                           region,
                           status,
                           capacity
                    FROM data_planes
                    WHERE region = $1
                      AND mode = 'shared'
                      AND status = 'active'
                    "#,
                    region.as_str()
                )
                .fetch_all(transaction.as_mut())
                .await
            }
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
    ) -> Result<Option<DataPlane>, CoreError> {
        let required_capacity = i64::from(required_capacity);
        let row = match (&self.executor, region) {
            (PgExecutor::Pool(pool), Some(region)) => {
                sqlx::query_as!(
                    DataPlaneRow,
                    r#"
                    SELECT dp.id,
                           dp.mode,
                           dp.region,
                           dp.status,
                           dp.capacity
                    FROM data_planes dp
                    LEFT JOIN deployments d
                      ON d.dataplane_id = dp.id
                     AND d.deleted_at IS NULL
                    WHERE dp.region = $1
                      AND dp.mode = 'shared'
                      AND dp.status = 'active'
                    GROUP BY dp.id, dp.mode, dp.region, dp.status, dp.capacity
                    HAVING (dp.capacity::BIGINT - COUNT(d.id)) >= $2
                    ORDER BY COUNT(d.id) ASC
                    LIMIT 1
                    "#,
                    region.as_str(),
                    required_capacity
                )
                .fetch_optional(*pool)
                .await
            }
            (PgExecutor::Pool(pool), None) => {
                sqlx::query_as!(
                    DataPlaneRow,
                    r#"
                    SELECT dp.id,
                           dp.mode,
                           dp.region,
                           dp.status,
                           dp.capacity
                    FROM data_planes dp
                    LEFT JOIN deployments d
                      ON d.dataplane_id = dp.id
                     AND d.deleted_at IS NULL
                    WHERE dp.mode = 'shared'
                      AND dp.status = 'active'
                    GROUP BY dp.id, dp.mode, dp.region, dp.status, dp.capacity
                    HAVING (dp.capacity::BIGINT - COUNT(d.id)) >= $1
                    ORDER BY COUNT(d.id) ASC
                    LIMIT 1
                    "#,
                    required_capacity
                )
                .fetch_optional(*pool)
                .await
            }
            (PgExecutor::Tx(tx), Some(region)) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query_as!(
                    DataPlaneRow,
                    r#"
                    SELECT dp.id,
                           dp.mode,
                           dp.region,
                           dp.status,
                           dp.capacity
                    FROM data_planes dp
                    LEFT JOIN deployments d
                      ON d.dataplane_id = dp.id
                     AND d.deleted_at IS NULL
                    WHERE dp.region = $1
                      AND dp.mode = 'shared'
                      AND dp.status = 'active'
                    GROUP BY dp.id, dp.mode, dp.region, dp.status, dp.capacity
                    HAVING (dp.capacity::BIGINT - COUNT(d.id)) >= $2
                    ORDER BY COUNT(d.id) ASC
                    LIMIT 1
                    "#,
                    region.as_str(),
                    required_capacity
                )
                .fetch_optional(transaction.as_mut())
                .await
            }
            (PgExecutor::Tx(tx), None) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query_as!(
                    DataPlaneRow,
                    r#"
                    SELECT dp.id,
                           dp.mode,
                           dp.region,
                           dp.status,
                           dp.capacity
                    FROM data_planes dp
                    LEFT JOIN deployments d
                      ON d.dataplane_id = dp.id
                     AND d.deleted_at IS NULL
                    WHERE dp.mode = 'shared'
                      AND dp.status = 'active'
                    GROUP BY dp.id, dp.mode, dp.region, dp.status, dp.capacity
                    HAVING (dp.capacity::BIGINT - COUNT(d.id)) >= $1
                    ORDER BY COUNT(d.id) ASC
                    LIMIT 1
                    "#,
                    required_capacity
                )
                .fetch_optional(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to find available data plane: {}", e),
        })?;

        row.map(|row| row.into_dataplane()).transpose()
    }

    async fn list_all(&self) -> Result<Vec<DataPlane>, CoreError> {
        let rows = match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query_as!(
                    DataPlaneRow,
                    r#"
                    SELECT id,
                           mode,
                           region,
                           status,
                           capacity
                    FROM data_planes
                    ORDER BY region ASC, id ASC
                    "#
                )
                .fetch_all(*pool)
                .await
            }
            PgExecutor::Tx(tx) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query_as!(
                    DataPlaneRow,
                    r#"
                    SELECT id,
                           mode,
                           region,
                           status,
                           capacity
                    FROM data_planes
                    ORDER BY region ASC, id ASC
                    "#
                )
                .fetch_all(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list data planes: {}", e),
        })?;

        rows.into_iter().map(|row| row.into_dataplane()).collect()
    }

    async fn current_load(&self, id: &DataPlaneId) -> Result<u32, CoreError> {
        let count: i64 = match &self.executor {
            PgExecutor::Pool(pool) => {
                sqlx::query_scalar!(
                    r#"
                    SELECT COUNT(*)::BIGINT as "count!"
                    FROM deployments
                    WHERE dataplane_id = $1
                      AND deleted_at IS NULL
                    "#,
                    id.0
                )
                .fetch_one(*pool)
                .await
            }
            PgExecutor::Tx(tx) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
                sqlx::query_scalar!(
                    r#"
                    SELECT COUNT(*)::BIGINT as "count!"
                    FROM deployments
                    WHERE dataplane_id = $1
                      AND deleted_at IS NULL
                    "#,
                    id.0
                )
                .fetch_one(transaction.as_mut())
                .await
            }
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
        match &self.executor {
            PgExecutor::Pool(pool) => {
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
                .execute(*pool)
                .await
            }
            PgExecutor::Tx(tx) => {
                let mut guard = tx.lock().await;
                let transaction = guard
                    .as_mut()
                    .ok_or_else(|| CoreError::InternalError("Transaction missing".to_string()))?;
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
                .execute(transaction.as_mut())
                .await
            }
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to save data plane: {}", e),
        })?;

        Ok(())
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
