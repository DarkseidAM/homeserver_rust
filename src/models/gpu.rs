// GPU model. Populated by gpu_repo (NVIDIA via NVML feature, AMD/Intel via /sys/class/drm).

use serde::{Deserialize, Serialize};
use wincode::{SchemaRead, SchemaWrite};

#[derive(Debug, Clone, Default, Serialize, Deserialize, SchemaRead, SchemaWrite)]
#[serde(rename_all = "camelCase")]
pub struct GpuStats {
    /// Zero-based index within its vendor backend.
    pub index: u32,
    /// "nvidia" | "amd" | "intel".
    pub vendor: String,
    pub name: String,
    pub utilization_percent: f64,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub temperature_c: f64,
    /// Board power draw in watts, when exposed.
    pub power_watts: Option<f64>,
    /// Fan speed as a percentage, when exposed.
    pub fan_percent: Option<f64>,
}
