// Aggregated snapshot: one row per time bucket (1-min or 5-min).
// Same blob types as raw snapshots; scalars have avg/min/max.

use serde::{Deserialize, Serialize};

use super::{ContainerStats, NetworkStats, StorageStats, SystemStatsDynamic};

/// One aggregated row: bucket start time, resolution, scalar aggregates, and blob data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregatedSnapshot {
    pub created_at: i64,
    pub resolution_seconds: i32,
    pub cpu_load_avg: f64,
    pub cpu_load_min: f64,
    pub cpu_load_max: f64,
    pub memory_used_avg: i64,
    pub memory_used_min: i64,
    pub memory_used_max: i64,
    pub containers: Vec<ContainerStats>,
    pub storage: StorageStats,
    pub network: NetworkStats,
    pub system: SystemStatsDynamic,
}
