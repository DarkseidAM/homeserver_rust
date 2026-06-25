// SMART disk-health model. Populated by smart_repo from `smartctl --json` output.

use serde::{Deserialize, Serialize};
use wincode::{SchemaRead, SchemaWrite};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, SchemaRead, SchemaWrite)]
#[serde(rename_all = "camelCase")]
pub struct SmartHealth {
    /// Device path, e.g. "/dev/sda".
    pub device: String,
    pub model: String,
    /// Overall SMART self-assessment (smartctl `smart_status.passed`).
    pub health_passed: bool,
    pub temperature_c: Option<i64>,
    pub power_on_hours: Option<u64>,
    /// Reallocated sector count (ATA attribute 5) — non-zero indicates wear.
    pub reallocated_sectors: Option<u64>,
    /// SSD/NVMe life used as a percentage (0 = new, 100 = rated life consumed).
    pub wear_level_percent: Option<u8>,
}
