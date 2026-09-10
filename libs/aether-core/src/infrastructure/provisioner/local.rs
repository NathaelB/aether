use tracing::info;

use crate::domain::{
    CoreError,
    dataplane::{
        entities::DataPlane,
        provisioner::{ClusterProvisioner, ProvisionRequest},
        value_objects::{Capacity, DataPlaneAllocation, DataPlaneId, DeploymentResources},
    },
};

/// Capacity given to the local data plane when nothing configures one.
///
/// Well above `DeploymentResources::DEFAULT` (500 millicores, 1Gi memory,
/// 1Gi storage): the single k3d cluster `local` tooling already brings up
/// runs Herald, Genesis and the operator alongside whatever gets deployed
/// into it, and a capacity equal to one deployment's default would leave no
/// room for a second.
const DEFAULT_LOCAL_CAPACITY: (u32, u32, u32) = (4_000, 8_192, 100);

/// Registers the already-running local data plane instead of creating one.
///
/// There is no cluster to create here: `local` tooling already produces one
/// k3d cluster the operator reconciles into, before this adapter is ever
/// asked to provision anything. `provision` therefore does not call out to
/// anything -- it builds the `DataPlane` record that points at the cluster
/// that already exists, which is the one piece of state the control plane is
/// actually missing. A real cloud adapter is where the infrastructure call
/// belongs.
#[derive(Debug, Clone, Copy)]
pub struct LocalClusterProvisioner {
    /// Injectable so a test can construct a small one rather than being stuck
    /// with the default.
    capacity: Capacity,
}

impl LocalClusterProvisioner {
    pub fn new(capacity: Capacity) -> Self {
        Self { capacity }
    }

    /// At least `minimum` in every dimension, honouring the port's contract
    /// ("sizes the cluster to at least this, and is free to size it larger")
    /// even though the configured capacity is usually already well above it.
    fn capacity_at_least(&self, minimum: DeploymentResources) -> Result<Capacity, CoreError> {
        Capacity::new(
            self.capacity.cpu_millis().max(minimum.cpu_millis),
            self.capacity.memory_mib().max(minimum.memory_mib),
            self.capacity.storage_gib().max(minimum.storage_gib),
        )
    }
}

impl Default for LocalClusterProvisioner {
    fn default() -> Self {
        let (cpu_millis, memory_mib, storage_gib) = DEFAULT_LOCAL_CAPACITY;
        let capacity = Capacity::new(cpu_millis, memory_mib, storage_gib)
            .expect("the default local capacity is non-zero in every dimension");

        Self::new(capacity)
    }
}

impl ClusterProvisioner for LocalClusterProvisioner {
    async fn provision(&self, request: ProvisionRequest) -> Result<DataPlane, CoreError> {
        let capacity = self.capacity_at_least(request.minimum)?;

        info!(
            organisation_id = %request.organisation_id.0,
            region = %request.region.as_str(),
            "registering the local data plane for a dedicated deployment"
        );

        Ok(DataPlane::new(
            DataPlaneAllocation::Dedicated {
                organisation_id: request.organisation_id,
            },
            request.region,
            capacity,
        ))
    }

    /// No-op, and correctly so: there is nothing this adapter created, so
    /// there is nothing for it to destroy. The k3d cluster outlives any one
    /// data plane record -- `local` tooling created it, and removing the
    /// `DataPlane` row (the repository's job, not this adapter's) is the
    /// entire cleanup for a dedicated allocation against it.
    async fn deprovision(&self, id: &DataPlaneId) -> Result<(), CoreError> {
        info!(dataplane_id = %id, "local provisioner has nothing to deprovision");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{dataplane::value_objects::Region, organisation::OrganisationId};
    use uuid::Uuid;

    fn small_capacity() -> Capacity {
        Capacity::new(500, 1_024, 1).expect("non-zero capacity")
    }

    /// The point of the adapter: it hands back a data plane the requesting
    /// organisation owns, in the region asked for -- not the region or owner
    /// of whatever cluster happens to be running locally.
    #[tokio::test]
    async fn the_local_provisioner_returns_a_plane_owned_by_the_requester_in_the_requested_region()
    {
        let provisioner = LocalClusterProvisioner::new(small_capacity());
        let organisation_id = OrganisationId(Uuid::new_v4());

        let dataplane = provisioner
            .provision(ProvisionRequest {
                organisation_id,
                region: Region::new("local"),
                minimum: DeploymentResources::DEFAULT,
            })
            .await
            .expect("provisioning never fails locally");

        assert_eq!(dataplane.allocation.owner(), Some(organisation_id));
        assert_eq!(dataplane.region.as_str(), "local");
    }

    /// Starts in `Provisioning`, exactly like any other freshly registered
    /// data plane -- this adapter earns no shortcut around the state machine
    /// just because there is nothing left to wait for.
    #[tokio::test]
    async fn a_freshly_provisioned_local_plane_has_never_reported() {
        let provisioner = LocalClusterProvisioner::new(small_capacity());

        let dataplane = provisioner
            .provision(ProvisionRequest {
                organisation_id: OrganisationId(Uuid::new_v4()),
                region: Region::new("local"),
                minimum: DeploymentResources::DEFAULT,
            })
            .await
            .expect("provisioning never fails locally");

        assert_eq!(dataplane.last_seen_at, None);
    }

    /// The contract's "at least": a deployment asking for more than the
    /// configured capacity still gets a plane that fits it, rather than one
    /// that silently cannot host what it was provisioned for.
    #[tokio::test]
    async fn provisioning_sizes_the_plane_to_at_least_the_requested_minimum() {
        let provisioner = LocalClusterProvisioner::new(small_capacity());
        let minimum = DeploymentResources::new(2_000, 4_096, 50).expect("valid resources");

        let dataplane = provisioner
            .provision(ProvisionRequest {
                organisation_id: OrganisationId(Uuid::new_v4()),
                region: Region::new("local"),
                minimum,
            })
            .await
            .expect("provisioning never fails locally");

        assert!(dataplane.capacity.cpu_millis() >= minimum.cpu_millis);
        assert!(dataplane.capacity.memory_mib() >= minimum.memory_mib);
        assert!(dataplane.capacity.storage_gib() >= minimum.storage_gib);
    }

    /// Deprovisioning is a no-op, but it must still be callable and succeed
    /// -- cleanup code that calls it unconditionally must not fail because
    /// this adapter never created anything.
    #[tokio::test]
    async fn deprovisioning_the_local_plane_always_succeeds() {
        let provisioner = LocalClusterProvisioner::new(small_capacity());

        let result = provisioner.deprovision(&DataPlaneId(Uuid::new_v4())).await;

        assert!(result.is_ok());
    }
}
