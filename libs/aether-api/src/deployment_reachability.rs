//! Periodic probe for deployment reachability.
//!
//! Checks each live deployment's health via its public hostname and opens/closes
//! a signal based on whether it responds. This is distinct from `IdentityInstanceStatusWatcher`
//! in genesis-core, which reports Kubernetes operator state: it sees DNS gaps, misconfigured
//! Ingress, and deadlocked apps where the pod still reports Ready.

use std::time::Duration;

use aether_core::deployments::ports::DeploymentService;
use aether_core::signals::{Signal, SignalId, SignalKind, SignalSubject};
use chrono::Utc;
use reqwest::Client;
use tokio::time::interval;
use tracing::{error, info};
use uuid::Uuid;

use crate::state::AppState;

/// How often to check deployment reachability.
const EVERY: Duration = Duration::from_secs(60);

/// Timeout for each HTTP health check.
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

/// Maps a deployment kind to its health check path.
///
/// These are assumptions that should be verified against the actual container
/// images before production deployment.
fn health_path_for_kind(kind: &aether_core::deployments::DeploymentKind) -> &'static str {
    match kind {
        aether_core::deployments::DeploymentKind::Ferriskey => "/health",
        aether_core::deployments::DeploymentKind::Keycloak => "/health/ready",
    }
}

/// Performs an HTTP HEAD or GET request to check if a deployment is reachable.
///
/// Returns `true` if the deployment answers with a 2xx status code.
async fn check_deployment_reachable(client: &Client, url: &str) -> bool {
    match client.get(url).timeout(HTTP_TIMEOUT).send().await {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

/// Runs the deployment reachability probe.
///
/// On a schedule, lists all live deployments, checks their health endpoints,
/// and opens/closes signals accordingly. Runs beside the server like other
/// background probes: nothing a caller does should be the thing that signals
/// an unreachable deployment, and one that went down before this started is
/// caught up on startup.
pub async fn run_deployment_reachability_probe(state: AppState) {
    info!("starting deployment reachability probe");

    let client = Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .expect("failed to build HTTP client");

    let mut ticker = interval(EVERY);

    loop {
        ticker.tick().await;

        let deployments = match state.service.list_all_live_deployments().await {
            Ok(deployments) => deployments,
            Err(err) => {
                error!(%err, "failed to list live deployments for reachability probe");
                continue;
            }
        };

        for deployment in deployments {
            // Get the DNS zone; if not configured, skip this deployment.
            let Some(zone) = state.args.ovh.domain() else {
                continue;
            };

            // Look up the organisation slug for this deployment.
            let organisation_slug = match state
                .service
                .organisation_slug(deployment.organisation_id)
                .await
            {
                Ok(Some(slug)) => slug,
                Ok(None) => {
                    error!(
                        deployment_id = %deployment.id,
                        "failed to look up organisation for deployment"
                    );
                    continue;
                }
                Err(err) => {
                    error!(
                        deployment_id = %deployment.id,
                        %err,
                        "failed to query organisation slug"
                    );
                    continue;
                }
            };

            // Build the hostname and construct the health check URL.
            let hostname =
                aether_core::dns::hostname_for(&organisation_slug, &deployment.name.0, &zone);
            let health_path = health_path_for_kind(&deployment.kind);
            let url = format!("https://{}{}", hostname, health_path);

            let now = Utc::now();
            let dedup_key = format!("deployment-unreachable-{}", deployment.id.0);

            // Check if the deployment is reachable.
            if check_deployment_reachable(&client, &url).await {
                // Deployment is reachable; close any open signal.
                if let Err(err) = state.service.close_signal(&dedup_key, now).await {
                    error!(
                        deployment_id = %deployment.id,
                        %err,
                        "failed to close deployment reachability signal"
                    );
                }
            } else {
                // Deployment is unreachable; open/update a signal.
                let message = format!(
                    "Deployment {} at {} did not respond to health check",
                    deployment.name.0, hostname
                );

                let signal = Signal::open(
                    SignalId(Uuid::new_v4()),
                    SignalKind::DeploymentUnreachable,
                    SignalSubject::Deployment { id: deployment.id },
                    dedup_key,
                    message,
                    now,
                );

                if let Err(err) = state.service.write_signal(signal).await {
                    error!(
                        deployment_id = %deployment.id,
                        %err,
                        "failed to write deployment reachability signal"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_core::deployments::DeploymentKind;

    /// Test that health path is determined correctly by kind.
    #[test]
    fn health_path_is_correct_per_kind() {
        assert_eq!(health_path_for_kind(&DeploymentKind::Ferriskey), "/health");
        assert_eq!(
            health_path_for_kind(&DeploymentKind::Keycloak),
            "/health/ready"
        );
    }

    /// Test that dedup key format is stable.
    #[test]
    fn dedup_key_format_is_stable() {
        let id = Uuid::nil();
        let dedup_key = format!("deployment-unreachable-{}", id);

        assert_eq!(
            dedup_key,
            "deployment-unreachable-00000000-0000-0000-0000-000000000000"
        );
    }
}
