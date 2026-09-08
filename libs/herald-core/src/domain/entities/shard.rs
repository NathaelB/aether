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

#[cfg(test)]
mod tests {
    use super::*;

    fn deployment_ids(count: usize) -> Vec<DeploymentId> {
        (0..count)
            .map(|i| DeploymentId::new(format!("deployment-{i}")))
            .collect()
    }

    #[test]
    fn every_deployment_is_owned_by_exactly_one_shard() {
        const SHARD_COUNT: usize = 3;
        let shards: Vec<ShardConfig> = (0..SHARD_COUNT)
            .map(|index| ShardConfig::new(index, SHARD_COUNT))
            .collect();

        for deployment_id in deployment_ids(50) {
            let owners: Vec<usize> = shards
                .iter()
                .filter(|shard| shard.owns_deployment(&deployment_id))
                .map(|shard| shard.shard_index)
                .collect();

            assert_eq!(
                owners.len(),
                1,
                "deployment {deployment_id} should be owned by exactly one of {SHARD_COUNT} shards, got {owners:?}"
            );
        }
    }

    #[test]
    fn ownership_is_stable_across_calls() {
        let shard = ShardConfig::new(1, 3);
        let deployment_id = DeploymentId::new("stable-deployment");

        let first = shard.owns_deployment(&deployment_id);
        for _ in 0..100 {
            assert_eq!(shard.owns_deployment(&deployment_id), first);
        }
    }

    #[test]
    #[should_panic(expected = "shard_count must be greater than 0")]
    fn rejects_zero_shard_count() {
        ShardConfig::new(0, 0);
    }

    #[test]
    #[should_panic(expected = "shard_index must be less than shard_count")]
    fn rejects_out_of_range_shard_index() {
        ShardConfig::new(3, 3);
    }
}
