use std::fmt::Display;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use chrono::{DateTime, Utc};

use crate::{CoreError, organisation::OrganisationId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct DataPlaneId(pub Uuid);

impl Display for DataPlaneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct Region(String);

/// What a *deployment* asks for. Intent, expressed before any data plane has
/// been chosen -- which is why it carries no organisation: at that point the
/// organisation comes from the route, not from the mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum DataPlaneMode {
    Shared,
    Dedicated,
}

/// What a *data plane* is.
///
/// Separate from [`DataPlaneMode`] on purpose. Conflating "what is wanted" with
/// "what exists" is what would force an `Option<OrganisationId>` next to a
/// `Dedicated` variant, and with it a runtime check that a dedicated data
/// plane really has an owner. Here a dedicated one cannot exist without one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum DataPlaneAllocation {
    /// Open to any organisation with room on it.
    Shared,
    /// Reserved to one organisation. Placement for anyone else must not see it.
    Dedicated { organisation_id: OrganisationId },
}

impl DataPlaneAllocation {
    /// The intent this allocation can satisfy.
    pub fn mode(&self) -> DataPlaneMode {
        match self {
            Self::Shared => DataPlaneMode::Shared,
            Self::Dedicated { .. } => DataPlaneMode::Dedicated,
        }
    }

    pub fn owner(&self) -> Option<OrganisationId> {
        match self {
            Self::Shared => None,
            Self::Dedicated { organisation_id } => Some(*organisation_id),
        }
    }

    /// Whether this data plane may host a deployment for `organisation_id`.
    pub fn accepts(&self, organisation_id: OrganisationId) -> bool {
        match self {
            Self::Shared => true,
            Self::Dedicated {
                organisation_id: owner,
            } => *owner == organisation_id,
        }
    }
}

/// Observed liveness, as opposed to `DataPlaneStatus`, which records what an
/// operator decided. A data plane drained for maintenance and one that stopped
/// answering call for different responses, so they are different types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum DataPlaneLiveness {
    /// Reported within the configured window.
    Reachable,
    /// Reported at some point, but not recently enough to be placed on.
    Unreachable,
    /// Never reported since it was registered.
    NeverSeen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum DataPlaneStatus {
    /// Being created. True the moment a cluster takes minutes to exist, which
    /// is why registering one no longer implies it can serve.
    Provisioning,
    Active,
    Draining,
    Disabled,
    /// Provisioning failed. Terminal until an operator acts; retrying forever
    /// would hide a quota or a credential problem behind a spinner.
    Failed,
}

impl Region {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// What a data plane has to give out, in the same units a deployment asks for.
///
/// It used to be a single count of deployments, which treated a freemium
/// instance on 1Gi and an enterprise one on 100Gi as costing the same. The
/// binding constraint on a cluster running an IAM workload and a Postgres
/// cluster per deployment is CPU, memory and disk -- not cardinality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct Capacity {
    cpu_millis: u32,
    memory_mib: u32,
    storage_gib: u32,
}

impl Capacity {
    pub fn new(cpu_millis: u32, memory_mib: u32, storage_gib: u32) -> Result<Self, CoreError> {
        if cpu_millis == 0 || memory_mib == 0 || storage_gib == 0 {
            return Err(CoreError::InvalidDataPlaneCapacity);
        }

        Ok(Self {
            cpu_millis,
            memory_mib,
            storage_gib,
        })
    }

    pub fn cpu_millis(&self) -> u32 {
        self.cpu_millis
    }

    pub fn memory_mib(&self) -> u32 {
        self.memory_mib
    }

    pub fn storage_gib(&self) -> u32 {
        self.storage_gib
    }

    /// Whether this capacity still fits `wanted` once `used` is accounted for.
    ///
    /// All three dimensions must fit: a data plane with spare CPU and no disk
    /// left cannot host a deployment whose database needs a volume.
    pub fn fits(&self, used: DeploymentResources, wanted: DeploymentResources) -> bool {
        self.cpu_millis >= used.cpu_millis.saturating_add(wanted.cpu_millis)
            && self.memory_mib >= used.memory_mib.saturating_add(wanted.memory_mib)
            && self.storage_gib >= used.storage_gib.saturating_add(wanted.storage_gib)
    }
}

pub struct CreateDataplaneCommand {
    pub region: Region,
    /// Shared, or dedicated to a named organisation. There is no way to ask
    /// for a dedicated data plane without saying whose it is.
    pub allocation: DataPlaneAllocation,
    pub capacity: Capacity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListDataPlaneDeploymentsCommand {
    pub shard_index: usize,
    pub shard_count: usize,
    pub limit: usize,
    pub cursor: Option<String>,
}

impl ListDataPlaneDeploymentsCommand {
    pub const DEFAULT_LIMIT: usize = 10;

    pub fn new(
        shard_index: Option<usize>,
        shard_count: Option<usize>,
        limit: Option<usize>,
        cursor: Option<String>,
    ) -> Result<Self, String> {
        let shard_count = shard_count.unwrap_or(1);
        if shard_count == 0 {
            return Err("shard_count must be greater than 0".to_string());
        }

        let shard_index = shard_index.unwrap_or(0);
        if shard_index >= shard_count {
            return Err("shard_index must be lower than shard_count".to_string());
        }

        let limit = limit.unwrap_or(Self::DEFAULT_LIMIT);
        if limit == 0 {
            return Err("limit must be greater than 0".to_string());
        }

        Ok(Self {
            shard_index,
            shard_count,
            limit,
            cursor,
        })
    }
}

/// What one deployment costs a data plane, and what its database is sized to.
///
/// The same value reserves room during placement and is written into the
/// `IdentityInstance` spec, so the two cannot drift. Before this, placement
/// counted deployments and Genesis invented the sizing at the other end --
/// which meant a data plane could accept a deployment it had no disk for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct DeploymentResources {
    /// CPU in millicores, matching Kubernetes' own unit.
    pub cpu_millis: u32,
    pub memory_mib: u32,
    pub storage_gib: u32,
}

impl DeploymentResources {
    /// The size a deployment gets when the request does not ask for one.
    ///
    /// These are the values `k8s/examples/identity-instance-ferriskey.yaml`
    /// already used, rather than a number invented here: 500m and 1Gi of
    /// memory for the database, on 1Gi of storage.
    pub const DEFAULT: Self = Self {
        cpu_millis: 500,
        memory_mib: 1024,
        storage_gib: 1,
    };

    pub fn new(cpu_millis: u32, memory_mib: u32, storage_gib: u32) -> Result<Self, CoreError> {
        if cpu_millis == 0 || memory_mib == 0 || storage_gib == 0 {
            return Err(CoreError::InvalidDeploymentResources {
                reason: "cpu, memory and storage must all be greater than zero".to_string(),
            });
        }

        Ok(Self {
            cpu_millis,
            memory_mib,
            storage_gib,
        })
    }
}

impl Default for DeploymentResources {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// How placement chooses between data planes that all have room.
///
/// This used to be an `ORDER BY COUNT(d.id) ASC` nobody read, which is
/// least-loaded — it spreads. Spreading gives each deployment more headroom;
/// packing leaves whole nodes empty so they can be scaled down, which is
/// usually where the money is. Naming it makes the trade a decision rather
/// than an accident of a query.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum PlacementPolicy {
    /// Least-loaded first. The behaviour before it had a name.
    #[default]
    Spread,
    /// Fullest-that-still-fits first.
    Pack,
}

#[cfg(test)]
mod capacity_tests {
    use super::*;

    fn capacity() -> Capacity {
        Capacity::new(4_000, 8_192, 100).expect("non-zero capacity")
    }

    fn nothing_used() -> DeploymentResources {
        DeploymentResources::new(0, 0, 0).unwrap_or(DeploymentResources {
            cpu_millis: 0,
            memory_mib: 0,
            storage_gib: 0,
        })
    }

    /// The point of the change. Under the old count these two were identical:
    /// each deployment cost exactly one.
    #[test]
    fn a_large_deployment_costs_more_than_a_small_one() {
        let small = DeploymentResources::new(500, 1_024, 1).expect("valid");
        let large = DeploymentResources::new(2_000, 4_096, 80).expect("valid");

        assert!(capacity().fits(nothing_used(), small));
        assert!(capacity().fits(nothing_used(), large));

        // Once the large one is placed, the small one no longer fits on disk
        // even though CPU and memory are still plentiful.
        assert!(!capacity().fits(large, large));
    }

    /// A cluster is bound by whichever dimension runs out first, and disk is
    /// usually the one -- an IAM workload is small and its database has a
    /// volume.
    #[test]
    fn spare_cpu_does_not_make_up_for_missing_disk() {
        let used = DeploymentResources::new(100, 256, 100).expect("valid");
        let wanted = DeploymentResources::new(100, 256, 1).expect("valid");

        assert!(
            capacity().cpu_millis() > used.cpu_millis + wanted.cpu_millis,
            "the test is meaningless unless CPU is genuinely spare"
        );
        assert!(!capacity().fits(used, wanted), "storage is exhausted");
    }

    #[test]
    fn a_capacity_that_exactly_fits_is_accepted() {
        let used = DeploymentResources::new(3_500, 7_168, 99).expect("valid");
        let wanted = DeploymentResources::new(500, 1_024, 1).expect("valid");

        assert!(capacity().fits(used, wanted), "exactly full still fits");
    }

    #[test]
    fn a_capacity_with_a_zero_dimension_is_rejected() {
        assert!(Capacity::new(0, 1_024, 10).is_err());
        assert!(Capacity::new(500, 0, 10).is_err());
        assert!(Capacity::new(500, 1_024, 0).is_err());
    }

    #[test]
    fn deployment_resources_reject_a_zero_dimension() {
        assert!(DeploymentResources::new(0, 1_024, 1).is_err());
        assert!(DeploymentResources::new(500, 0, 1).is_err());
        assert!(DeploymentResources::new(500, 1_024, 0).is_err());
    }

    /// The default is the size the repository's own example already used, so
    /// changing it changes what an unspecified deployment gets -- which is why
    /// it is asserted rather than left to drift.
    #[test]
    fn the_default_size_matches_the_repository_example() {
        assert_eq!(DeploymentResources::DEFAULT.cpu_millis, 500);
        assert_eq!(DeploymentResources::DEFAULT.memory_mib, 1_024);
        assert_eq!(DeploymentResources::DEFAULT.storage_gib, 1);
    }

    /// Spreading is the behaviour that existed before it had a name; this
    /// pins it so a change to the default is a deliberate act.
    #[test]
    fn placement_spreads_unless_told_otherwise() {
        assert_eq!(PlacementPolicy::default(), PlacementPolicy::Spread);
    }
}

/// Everything placement needs to choose a data plane.
///
/// Grouped rather than passed as six positional arguments: `find_available`
/// had grown a region, a mode, a size, a policy and a liveness threshold, and
/// adding the owning organisation to that list is where a caller starts
/// swapping two of them by accident.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementRequest {
    /// `None` places anywhere. Callers that carry a region always pass it.
    pub region: Option<Region>,
    /// Whose deployment this is. A dedicated data plane belonging to anyone
    /// else must not be considered.
    pub organisation_id: OrganisationId,
    pub mode: DataPlaneMode,
    pub resources: DeploymentResources,
    pub policy: PlacementPolicy,
    /// A data plane that has not reported since this instant is treated as
    /// gone.
    pub seen_since: DateTime<Utc>,
}

#[cfg(test)]
mod allocation_tests {
    use super::*;
    use uuid::Uuid;

    fn org() -> OrganisationId {
        OrganisationId(Uuid::new_v4())
    }

    /// The reason the allocation exists. Under the old `DataPlaneMode`,
    /// "dedicated" only meant "not shared" -- there was nothing to compare
    /// against, so nothing stopped one organisation's deployment landing on
    /// another's reserved cluster.
    #[test]
    fn a_dedicated_data_plane_only_accepts_its_owner() {
        let owner = org();
        let someone_else = org();
        let allocation = DataPlaneAllocation::Dedicated {
            organisation_id: owner,
        };

        assert!(allocation.accepts(owner));
        assert!(!allocation.accepts(someone_else));
    }

    #[test]
    fn a_shared_data_plane_accepts_anyone() {
        assert!(DataPlaneAllocation::Shared.accepts(org()));
        assert!(DataPlaneAllocation::Shared.accepts(org()));
    }

    /// A shared allocation has no variant that could carry an owner, so the
    /// contradiction the old shape allowed -- shared *and* owned -- is not
    /// expressible rather than merely rejected.
    #[test]
    fn only_a_dedicated_allocation_has_an_owner() {
        let owner = org();

        assert_eq!(DataPlaneAllocation::Shared.owner(), None);
        assert_eq!(
            DataPlaneAllocation::Dedicated {
                organisation_id: owner
            }
            .owner(),
            Some(owner)
        );
    }

    /// Placement asks for a mode; a data plane has an allocation. The mapping
    /// between them is what lets a `Dedicated` request match a data plane
    /// dedicated to the right organisation.
    #[test]
    fn an_allocation_reports_the_mode_it_satisfies() {
        assert_eq!(DataPlaneAllocation::Shared.mode(), DataPlaneMode::Shared);
        assert_eq!(
            DataPlaneAllocation::Dedicated {
                organisation_id: org()
            }
            .mode(),
            DataPlaneMode::Dedicated
        );
    }
}
