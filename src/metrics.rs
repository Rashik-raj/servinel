use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServiceMetrics {
    pub cpu: f32,
    pub memory: u64,
    #[serde(default)]
    pub memory_total: u64,
}
