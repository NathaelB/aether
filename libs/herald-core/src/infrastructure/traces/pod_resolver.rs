//! Attributing an inbound OTLP push to the deployment it came from.
//!
//! Traces arrive pushed, unlike logs: nothing here asked FerrisKey to send
//! this batch, so the only fact to check is *where it came from*. The
//! answer relies on the same trust boundary log shipping already does --
//! whatever pod matches a deployment's own labels, in the cluster Herald
//! already has API access to, is that deployment (see
//! `infrastructure/logs/kubernetes.rs::selector`) -- read backwards here,
//! from a source IP to the pod holding it, rather than forwards from a
//! label selector.
//!
//! This trusts the TCP source address as the sending pod's own IP, which
//! holds for ordinary pod-to-ClusterIP-Service traffic on the CNIs Aether
//! targets (no SNAT between a pod and a Service in the same cluster) but is
//! not a cryptographic guarantee -- deliberately paired with the
//! `NetworkPolicy` restricting this port to in-cluster traffic (see the
//! chart), the same way the log-reading path's trust boundary is Kubernetes
//! RBAC rather than a bearer token.

use std::collections::HashMap;
use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::time::{Duration, Instant};

use k8s_openapi::api::core::v1::Pod;
use kube::Client;
use kube::api::{Api, ListParams};
use tokio::sync::Mutex;
use tracing::debug;

use crate::domain::deployment_registry::DeploymentRegistry;
use crate::domain::entities::deployment::DeploymentId;
use crate::domain::entities::logs::OrganisationId;
use crate::domain::error::HeraldError;
use crate::domain::ports::DeploymentResolver;

/// How long a resolved IP is trusted before being looked up again.
///
/// A pod's IP is stable for its own lifetime; this only has to survive a
/// restart cycling the same IP to a different pod within one interval, which
/// a value this far under Kubernetes' usual restart cadence makes unlikely
/// to matter.
const CACHE_TTL: Duration = Duration::from_secs(60);

/// How long listing pods by IP may take before this span is dropped rather
/// than held open -- the same reasoning as `LIST_TIMEOUT` in
/// `infrastructure/logs/kubernetes.rs`.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);

const INSTANCE_LABEL: &str = "app.kubernetes.io/instance";
const DEPLOYMENT_PREFIX: &str = "deployment-";

/// Resolves a source IP to `(organisation_id, deployment_id)` by listing
/// pods cluster-wide -- unlike `KubePodLogSource`, which already knows the
/// namespace it is looking in, this does not, since a span arrives with
/// nothing but a source address.
pub struct KubeDeploymentResolver {
    client: Client,
    registry: DeploymentRegistry,
    cache: Mutex<HashMap<IpAddr, (DeploymentId, Instant)>>,
}

impl KubeDeploymentResolver {
    pub fn new(client: Client, registry: DeploymentRegistry) -> Self {
        Self {
            client,
            registry,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Builds a client from the ambient configuration, the same way
    /// [`crate::infrastructure::logs::kubernetes::KubePodLogSource::from_env`]
    /// does -- kept here rather than in `apps/herald` so that crate never
    /// needs `kube` as a direct dependency of its own.
    pub async fn from_env(registry: DeploymentRegistry) -> Result<Self, HeraldError> {
        let client = Client::try_default()
            .await
            .map_err(|error| HeraldError::Internal {
                message: format!("failed to build a Kubernetes client: {error}"),
            })?;

        Ok(Self::new(client, registry))
    }

    async fn deployment_id_for_ip(&self, source_ip: IpAddr) -> Option<DeploymentId> {
        if let Some((deployment_id, seen_at)) = self.cache.lock().await.get(&source_ip)
            && seen_at.elapsed() < CACHE_TTL
        {
            return Some(deployment_id.clone());
        }

        let pods: Api<Pod> = Api::all(self.client.clone());
        let params = ListParams::default().fields(&format!("status.podIP={source_ip}"));

        let listed = tokio::time::timeout(LOOKUP_TIMEOUT, pods.list(&params))
            .await
            .ok()?
            .ok()?;

        let deployment_id = listed
            .items
            .first()?
            .metadata
            .labels
            .as_ref()?
            .get(INSTANCE_LABEL)?
            .strip_prefix(DEPLOYMENT_PREFIX)
            .map(DeploymentId::new)?;

        self.cache
            .lock()
            .await
            .insert(source_ip, (deployment_id.clone(), Instant::now()));

        Some(deployment_id)
    }
}

impl DeploymentResolver for KubeDeploymentResolver {
    fn resolve<'a>(
        &'a self,
        source_ip: IpAddr,
    ) -> Pin<Box<dyn Future<Output = Option<(OrganisationId, DeploymentId)>> + Send + 'a>> {
        Box::pin(async move {
            let deployment_id = self.deployment_id_for_ip(source_ip).await?;
            let organisation_id = self.registry.organisation_of(&deployment_id).await?;
            debug!(%deployment_id, %organisation_id, %source_ip, "attributed an inbound span batch");
            Some((organisation_id, deployment_id))
        })
    }
}
