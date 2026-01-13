// API Group
pub const API_GROUP: &str = "aether.io";

// API Version
pub const API_VERSION: &str = "v1alpha1";

// Condition types
pub const CONDITION_READY: &str = "Ready";
pub const CONDITION_AVAILABLE: &str = "Available";
pub const CONDITION_PROGRESSING: &str = "Progressing";
pub const CONDITION_DEGRADED: &str = "Degraded";

// Reasons
pub const REASON_DEPLOYING: &str = "Deploying";
pub const REASON_DEPLOYED: &str = "Deployed";
pub const REASON_FAILED: &str = "Failed";
pub const REASON_UPDATING: &str = "Updating";
pub const REASON_DELETING: &str = "Deleting";

// Default values
pub const DEFAULT_REPLICAS: i32 = 1;
pub const DEFAULT_CPU_REQUEST: &str = "500m";
pub const DEFAULT_MEMORY_REQUEST: &str = "1Gi";
pub const DEFAULT_CPU_LIMIT: &str = "2000m";
pub const DEFAULT_MEMORY_LIMIT: &str = "2Gi";
