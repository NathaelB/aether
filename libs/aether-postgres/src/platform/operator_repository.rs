use aether_domain::{
    CoreError,
    platform::{PlatformOperator, PlatformRight, PlatformRights, ports::OperatorRepository},
};
use aether_macros::repository;
use aether_persistence::SharedTx;
use chrono::{DateTime, Utc};
use sqlx::FromRow;

#[derive(FromRow)]
struct OperatorRow {
    subject: String,
    rights: Vec<String>,
    granted_by: Option<String>,
    granted_at: DateTime<Utc>,
}

impl OperatorRow {
    fn into_operator(self) -> Result<PlatformOperator, CoreError> {
        // Parsed rather than tolerated. A right the code does not know about
        // means the column and the domain have drifted, and carrying an
        // unreadable grant further in would answer "no" to a question nobody
        // asked properly.
        let rights = self
            .rights
            .iter()
            .map(|right| right.parse::<PlatformRight>())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(PlatformOperator {
            subject: self.subject,
            rights: PlatformRights::of(rights),
            granted_by: self.granted_by,
            granted_at: self.granted_at,
        })
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[repository(domain = PlatformOperator, backend = Postgres)]
pub struct PostgresOperatorRepository<'tx> {
    tx: SharedTx<'tx>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'tx> PostgresOperatorRepository<'tx> {
    pub fn new(tx: &SharedTx<'tx>) -> Self {
        Self { tx: tx.clone() }
    }
}

impl OperatorRepository for PostgresOperatorRepository<'_> {
    async fn find(&self, subject: &str) -> Result<Option<PlatformOperator>, CoreError> {
        let row = {
            let mut tx = self.tx.lock().await;
            sqlx::query_as!(
                OperatorRow,
                r#"
            SELECT subject, rights, granted_by, granted_at
            FROM platform_operators
            WHERE subject = $1
            "#,
                subject
            )
            .fetch_optional(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to read the operator: {e}"),
        })?;

        row.map(OperatorRow::into_operator).transpose()
    }

    async fn list(&self) -> Result<Vec<PlatformOperator>, CoreError> {
        let rows = {
            let mut tx = self.tx.lock().await;
            sqlx::query_as!(
                OperatorRow,
                r#"
            SELECT subject, rights, granted_by, granted_at
            FROM platform_operators
            ORDER BY granted_at ASC, subject ASC
            "#
            )
            .fetch_all(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list the operators: {e}"),
        })?;

        rows.into_iter().map(OperatorRow::into_operator).collect()
    }

    async fn grant(&self, operator: PlatformOperator) -> Result<(), CoreError> {
        let rights: Vec<String> = operator.rights.iter().map(|r| r.to_string()).collect();

        let mut tx = self.tx.lock().await;
        sqlx::query!(
            r#"
            INSERT INTO platform_operators (subject, rights, granted_by, granted_at)
            VALUES ($1, $2, $3, $4)
            -- One row per subject, so a second grant replaces the first rather
            -- than colliding. Narrowing and widening are then the same call,
            -- and there is no pair of them that can disagree about the result.
            ON CONFLICT (subject) DO UPDATE
                SET rights = EXCLUDED.rights,
                    granted_by = EXCLUDED.granted_by,
                    granted_at = EXCLUDED.granted_at
            "#,
            operator.subject,
            &rights,
            operator.granted_by,
            operator.granted_at
        )
        .execute(&mut ***tx)
        .await
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to grant the operator: {e}"),
        })?;

        Ok(())
    }

    async fn revoke(&self, subject: &str) -> Result<(), CoreError> {
        let mut tx = self.tx.lock().await;
        sqlx::query!("DELETE FROM platform_operators WHERE subject = $1", subject)
            .execute(&mut ***tx)
            .await
            .map_err(|e| CoreError::DatabaseError {
                message: format!("Failed to revoke the operator: {e}"),
            })?;

        Ok(())
    }

    async fn holders_of(&self, right: PlatformRight) -> Result<usize, CoreError> {
        let counted = {
            let mut tx = self.tx.lock().await;
            sqlx::query_scalar!(
                r#"
            SELECT COUNT(*) AS "held!"
            FROM platform_operators
            WHERE $1 = ANY(rights)
            "#,
                right.to_string()
            )
            .fetch_one(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to count the operators: {e}"),
        })?;

        Ok(counted.max(0) as usize)
    }
}
