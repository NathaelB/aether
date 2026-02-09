use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::deployment::DeploymentId;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShardConfig {
    pub shard_index: usize,
    pub shard_count: usize,
}

impl ShardConfig {
    pub fn new(shard_index: usize, shard_count: usize) -> Self {
        assert!(shard_count > 0, "shard_count must be greater than 0");
        assert!(
            shard_index < shard_count,
            "shard_index must be less than shard_count"
        );

        Self {
            shard_index,
            shard_count,
        }
    }

    /// Uses deterministic hashing: shard = hash(deployment_id) % shard_count
    pub fn owns_deployment(&self, deployment_id: &DeploymentId) -> bool {
        self.compute_shard(deployment_id) == self.shard_index
    }

    fn compute_shard(&self, deployment_id: &DeploymentId) -> usize {
        let mut hasher = DefaultHasher::new();
        deployment_id.hash(&mut hasher);
        let hash_value = hasher.finish();

        (hash_value % self.shard_count as u64) as usize
    }
}
