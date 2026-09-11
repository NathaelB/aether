use std::str::FromStr;

use sqlx::FromRow;
use uuid::Uuid;

use aether_domain::{
    CoreError,
    catalog::ports::{RolloutEstateEntry, RolloutEstateRepository},
    deployments::{DeploymentId, DeploymentKind},
    organisation::{OrganisationId, value_objects::Plan},
};
use aether_persistence::SharedTx;

#[derive(FromRow)]
struct RolloutEstateRow {
    deployment_id: Uuid,
    organisation_id: Uuid,
    plan: String,
}

impl RolloutEstateRow {
    fn into_entry(self) -> Result<RolloutEstateEntry, CoreError> {
        Ok(RolloutEstateEntry {
            deployment_id: DeploymentId(self.deployment_id),
            organisation_id: OrganisationId(self.organisation_id),
            plan: Plan::from_str(&self.plan)?,
        })
    }
}

/// Not registered through `#[repository]`: nothing else in the platform needs
/// "every live deployment of a product, with its organisation's plan", so
/// this is built directly from the ambient transaction where the rollout
/// coverage preview needs it, the same way the `transactional` macro's own
/// docs describe building a second repository instance.
#[cfg_attr(coverage_nightly, coverage(off))]
pub struct PostgresRolloutEstateRepository<'tx> {
    tx: SharedTx<'tx>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<'tx> PostgresRolloutEstateRepository<'tx> {
    pub fn new(tx: &SharedTx<'tx>) -> Self {
        Self { tx: tx.clone() }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl RolloutEstateRepository for PostgresRolloutEstateRepository<'_> {
    async fn list_for_rollout(
        &self,
        kind: &DeploymentKind,
    ) -> Result<Vec<RolloutEstateEntry>, CoreError> {
        let kind = kind.to_string();

        let rows = {
            let mut tx = self.tx.lock().await;
            sqlx::query_as!(
                RolloutEstateRow,
                r#"
            SELECT d.id AS deployment_id, d.organisation_id, o.plan
            FROM deployments d
            JOIN organisations o ON o.id = d.organisation_id
            WHERE d.kind = $1
              AND d.deleted_at IS NULL
            "#,
                kind
            )
            .fetch_all(&mut ***tx)
            .await
        }
        .map_err(|e| CoreError::DatabaseError {
            message: format!("Failed to list the estate for a rollout preview: {e}"),
        })?;

        rows.into_iter().map(RolloutEstateRow::into_entry).collect()
    }
}
