//! Publishing a deployment's own DNS record.
//!
//! Two moments write one: placement, where the address is usually already
//! known, and deletion, which always removes it outright. A sweep beside the
//! server catches up whatever placement could not -- most often a dedicated
//! data plane that had not heartbeated yet when its first deployment landed.
//!
//! Best-effort throughout, like the object store and key manager next door:
//! an installation that never configured OVH gets no hostname for any
//! deployment, and one that did gets a hostname a little late rather than a
//! failed request.

use std::time::Duration;

use aether_core::{
    deployments::Deployment,
    dns::{DnsProvider, hostname_for},
};
use aether_ovh::OvhDnsProvider;
use tokio::time::interval;
use tracing::{error, info, warn};

use crate::state::AppState;

/// How often the sweep catches up records placement could not write yet.
///
/// Minutes, not seconds: a hostname arriving a little after a dedicated data
/// plane's first heartbeat is expected, and a tighter interval only spends
/// more of this installation's OVH rate limit finding out nothing changed.
const SWEEP_EVERY: Duration = Duration::from_secs(5 * 60);

fn provider(state: &AppState) -> Option<OvhDnsProvider> {
    let config = state.args.ovh.config()?;

    match OvhDnsProvider::new(config) {
        Ok(provider) => Some(provider),
        Err(error) => {
            error!(%error, "the DNS provider client could not be built");
            None
        }
    }
}

/// Creates or updates one deployment's own record, if this installation
/// publishes DNS and its data plane's address is already known.
///
/// Meant to be spawned rather than awaited by the caller whose response does
/// not depend on it: a deployment is placed whether or not this succeeds,
/// and a data plane with no address yet is caught up by
/// [`reconcile_dns_records`] instead of blocking placement on it.
pub async fn reconcile_deployment(state: AppState, deployment: Deployment) {
    let Some(provider) = provider(&state) else {
        return;
    };

    let address = match state
        .service
        .dataplane_gateway_address(deployment.dataplane_id)
        .await
    {
        Ok(Some(address)) => address,
        // Not known yet, or the read itself failed -- either way there is
        // nothing to point a record at right now, and the sweep will find it
        // once there is.
        Ok(None) => return,
        Err(error) => {
            warn!(%error, deployment_id = %deployment.id, "could not look up this deployment's data plane");
            return;
        }
    };

    let hostname = hostname_for(&deployment.name.0, provider.zone());

    match provider.upsert_record(&hostname, &address).await {
        Ok(()) => info!(%hostname, %address, "published a DNS record"),
        Err(error) => warn!(%hostname, %error, "could not publish a DNS record yet"),
    }
}

/// Removes one deployment's own record. Same shape as
/// [`reconcile_deployment`]: spawned, best-effort, never something a caller
/// waits on.
pub async fn remove_deployment(state: AppState, deployment: Deployment) {
    let Some(provider) = provider(&state) else {
        return;
    };

    let hostname = hostname_for(&deployment.name.0, provider.zone());

    match provider.delete_record(&hostname).await {
        Ok(()) => info!(%hostname, "removed a DNS record"),
        Err(error) => warn!(%hostname, %error, "could not remove a DNS record"),
    }
}

/// Catches up whatever [`reconcile_deployment`] could not do immediately.
///
/// Runs beside the server, the same shape as `purge_deleted_deployments`:
/// nothing a caller does should be the thing that finally gets a deployment
/// its hostname. Exits rather than looping when this installation was never
/// given a zone -- there is nothing for it to ever catch up.
pub async fn reconcile_dns_records(state: AppState) {
    let Some(provider) = provider(&state) else {
        info!("no DNS zone is configured: deployments will get no hostname of their own");
        return;
    };

    info!(zone = provider.zone(), "reconciling DNS records");

    let mut ticker = interval(SWEEP_EVERY);

    loop {
        ticker.tick().await;

        let targets = match state.service.dns_targets().await {
            Ok(targets) => targets,
            Err(error) => {
                error!(%error, "could not list this installation's DNS targets");
                continue;
            }
        };

        for (deployment, address) in targets {
            let hostname = hostname_for(&deployment.name.0, provider.zone());

            if let Err(error) = provider.upsert_record(&hostname, &address).await {
                warn!(%hostname, %error, "could not publish a DNS record during reconciliation");
            }
        }
    }
}
