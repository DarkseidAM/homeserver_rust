// Storage / disk models

use serde::{Deserialize, Serialize};
use wincode::{SchemaRead, SchemaWrite};

#[derive(Debug, Clone, Serialize, Deserialize, SchemaRead, SchemaWrite)]
#[serde(rename_all = "camelCase")]
pub struct PartitionStat {
    pub mount: String,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub total_space: u64,
    pub used_space: u64,
    pub available_space: u64,
    pub usage_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, SchemaRead, SchemaWrite)]
#[serde(rename_all = "camelCase")]
pub struct DiskDeviceStat {
    pub name: String,
    pub model: String,
    pub size: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub transfer_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, SchemaRead, SchemaWrite)]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    pub partitions: Vec<PartitionStat>,
    pub disks: Vec<DiskDeviceStat>,
}
