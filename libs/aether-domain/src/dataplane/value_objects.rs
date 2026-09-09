use std::fmt::Display;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::CoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct DataPlaneId(pub Uuid);

impl Display for DataPlaneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct Region(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum DataPlaneMode {
    Shared,
    Dedicated,
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
    Active,
    Draining,
    Disabled,
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
    pub mode: DataPlaneMode,
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
