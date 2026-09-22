//! Which organisation owns which deployment, as of the shard's last sync
//! cycle.
//!
//! The one fact the trace pipeline's IP-to-pod resolver needs that a pod's
//! own labels never carry: `infrastructure/logs/kubernetes.rs::selector`
//! keys a pod to a deployment by `deployment_id` alone, and Herald otherwise
//! only ever learns `organisation_id` from a request the control plane
//! already built (see [`super::entities::logs::LogStreamRequest`]). An
//! inbound OTLP push carries neither, so this is where the answer comes
//! from instead.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use super::entities::deployment::{Deployment, DeploymentId};
use super::entities::logs::OrganisationId;

#[derive(Clone, Default)]
pub struct DeploymentRegistry {
    by_deployment: Arc<Mutex<HashMap<DeploymentId, OrganisationId>>>,
}

impl DeploymentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the whole table with what this cycle's sweep just saw.
    ///
    /// Wholesale, not merged: a deployment resharded away or deleted stops
    /// resolving here in the same cycle it stops owning a log reader (see
    /// `HeraldServiceImpl::reconcile_log_shipping`), rather than lingering
    /// until something else notices.
    pub async fn replace(&self, deployments: &[Deployment]) {
        let mut table = self.by_deployment.lock().await;
        table.clear();
        table.extend(
            deployments
                .iter()
                .map(|deployment| (deployment.id.clone(), deployment.organisation_id.clone())),
        );
    }

    pub async fn organisation_of(&self, deployment_id: &DeploymentId) -> Option<OrganisationId> {
        self.by_deployment.lock().await.get(deployment_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::dataplane::DataPlaneId;

    fn deployment(id: &str, organisation_id: &str) -> Deployment {
        Deployment {
            id: DeploymentId::new(id),
            dataplane_id: DataPlaneId::new("dp-1"),
            organisation_id: OrganisationId::new(organisation_id),
            name: "acme-prod".to_string(),
            kind: None,
            namespace: None,
            log_shipping_enabled: false,
        }
    }

    #[tokio::test]
    async fn a_known_deployment_resolves_to_its_organisation() {
        let registry = DeploymentRegistry::new();
        registry.replace(&[deployment("dep-1", "org-1")]).await;

        assert_eq!(
            registry.organisation_of(&DeploymentId::new("dep-1")).await,
            Some(OrganisationId::new("org-1"))
        );
    }

    #[tokio::test]
    async fn an_unknown_deployment_resolves_to_nothing() {
        let registry = DeploymentRegistry::new();
        registry.replace(&[deployment("dep-1", "org-1")]).await;

        assert_eq!(
            registry.organisation_of(&DeploymentId::new("dep-2")).await,
            None
        );
    }

    /// A deployment resharded away in a later cycle must stop resolving in
    /// that same cycle, not linger from a stale entry the replace forgot to
    /// clear.
    #[tokio::test]
    async fn a_replace_drops_a_deployment_no_longer_present() {
        let registry = DeploymentRegistry::new();
        registry.replace(&[deployment("dep-1", "org-1")]).await;
        registry.replace(&[deployment("dep-2", "org-2")]).await;

        assert_eq!(
            registry.organisation_of(&DeploymentId::new("dep-1")).await,
            None
        );
        assert_eq!(
            registry.organisation_of(&DeploymentId::new("dep-2")).await,
            Some(OrganisationId::new("org-2"))
        );
    }
}
