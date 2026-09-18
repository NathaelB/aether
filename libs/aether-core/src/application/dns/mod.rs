//! What DNS reconciliation needs to know, read without an identity to check.
//!
//! Publishing a record is not something a caller asks for -- it follows from
//! a deployment existing and its data plane having an address, the same way
//! purging deleted deployments follows from a retention window rather than a
//! request. The reads below serve that: [`AetherService::dataplane_gateway_address`]
//! answers "can this one deployment's record be written right now", asked
//! once at placement, and [`AetherService::dns_targets`] answers "everything
//! that should have a record right now", asked on a sweep. Neither takes an
//! [`aether_auth::Identity`] for the same reason `purge_deleted_deployments`
//! does not: nobody is the caller, the installation's own upkeep is.
//!
//! Turning what these return into an actual record is `aether-ovh`'s job,
//! wired in at `aether-api` -- this crate never holds a `DnsProvider`, the
//! same way it never held an object store or a key manager.

use aether_domain::{
    CoreError,
    dataplane::{ports::DataPlaneRepository, value_objects::DataPlaneId},
    deployments::{Deployment, DeploymentStatus, ports::DeploymentRepository},
    organisation::{OrganisationId, ports::OrganisationRepository},
};
use aether_macros::transactional;

use crate::AetherService;

impl AetherService {
    /// Where a data plane's own Gateway answers, if it has reported one yet.
    #[transactional(data_plane)]
    pub async fn dataplane_gateway_address(
        &self,
        dataplane_id: DataPlaneId,
    ) -> Result<Option<String>, CoreError> {
        Ok(data_plane_repository
            .find_by_id(&dataplane_id)
            .await?
            .and_then(|dataplane| dataplane.gateway_address))
    }

    /// The slug a deployment's own hostname is scoped under.
    ///
    /// The discriminator [`aether_domain::dns::hostname_for`] needs: it is
    /// what keeps two organisations' same-named deployments from fighting
    /// over one DNS record. `None` only if the organisation itself is gone,
    /// which is not a reason to fail a heartbeat-driven reconcile -- there is
    /// simply no record to write until that is no longer true.
    #[transactional(organisation)]
    pub async fn organisation_slug(
        &self,
        organisation_id: OrganisationId,
    ) -> Result<Option<String>, CoreError> {
        Ok(organisation_repository
            .find_by_id(&organisation_id)
            .await?
            .map(|organisation| organisation.slug.to_string()))
    }

    /// Every deployment that should have a DNS record right now, paired with
    /// the address it should point at and the organisation slug its hostname
    /// is scoped under.
    ///
    /// A data plane with no known address yet contributes nothing here
    /// rather than a target with an empty address: there is no record to
    /// point at nowhere, only one not written yet. A deployment already
    /// tearing down is left out the same way -- its record was removed when
    /// deletion was asked for, and a sweep finding it again is not a reason
    /// to bring it back. A deployment whose organisation cannot be found is
    /// left out too, the same way a data plane with no address is: nothing
    /// downstream can turn it into a hostname yet.
    #[transactional(data_plane, deployment, organisation)]
    pub async fn dns_targets(&self) -> Result<Vec<(Deployment, String, String)>, CoreError> {
        let dataplanes = data_plane_repository.list_all().await?;
        let mut targets = Vec::new();

        for dataplane in dataplanes {
            let Some(address) = dataplane.gateway_address else {
                continue;
            };

            let deployments = deployment_repository
                .list_by_dataplane(&dataplane.id)
                .await?;

            for deployment in deployments {
                if deployment.status == DeploymentStatus::Deleting {
                    continue;
                }

                let Some(organisation) = organisation_repository
                    .find_by_id(&deployment.organisation_id)
                    .await?
                else {
                    continue;
                };

                targets.push((deployment, address.clone(), organisation.slug.to_string()));
            }
        }

        Ok(targets)
    }
}
