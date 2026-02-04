// Domain models (ported from shared Kotlin)

use serde::{Deserialize, Serialize};
use wincode::{SchemaRead, SchemaWrite};

#[derive(Debug, Clone, Serialize, Deserialize, SchemaRead, SchemaWrite)]
#[serde(rename_all = "camelCase")]
pub struct CpuStats {
    pub model: String,
    pub physical_cores: u32,
    pub logical_cores: u32,
    pub usage_percent: f64,
    pub temperature: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, SchemaRead, SchemaWrite)]
#[serde(rename_all = "camelCase")]
pub struct RamStats {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub usage_percent: f64,
}

/// Docker container state; serializes to lowercase JSON (e.g. "running").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, SchemaRead, SchemaWrite)]
#[serde(rename_all = "lowercase")]
pub enum ContainerState {
    Running,
    Exited,
    Paused,
    Restarting,
    #[serde(other)]
    Unknown,
}

impl ContainerState {
    /// Parse from Docker API state string (e.g. "running", "exited").
    pub fn from_docker(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "running" => ContainerState::Running,
            "exited" => ContainerState::Exited,
            "paused" => ContainerState::Paused,
            "restarting" => ContainerState::Restarting,
            _ => ContainerState::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, SchemaRead, SchemaWrite)]
#[serde(rename_all = "camelCase")]
pub struct ContainerStats {
    pub id: String,
    pub name: String,
    pub cpu_percent: f64,
    pub memory_usage_bytes: u64,
    pub memory_limit_bytes: u64,
    pub state: ContainerState,
    #[serde(default)]
    pub network_rx_bytes: u64,
    #[serde(default)]
    pub network_tx_bytes: u64,
    #[serde(default)]
    pub block_read_bytes: u64,
    #[serde(default)]
    pub block_write_bytes: u64,
    #[serde(default)]
    pub pids: u64,
    #[serde(default)]
    pub cpu_throttled: bool,
    #[serde(default)]
    pub memory_max_usage_bytes: u64,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, SchemaRead, SchemaWrite)]
#[serde(rename_all = "camelCase")]
pub struct InterfaceStat {
    pub name: String,
    pub display_name: String,
    pub mac_address: String,
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
    pub packets_sent: u64,
    pub packets_recv: u64,
    pub speed: u64,
    /// Receive rate in bytes/sec (computed from previous snapshot).
    #[serde(default)]
    pub received_bytes_per_sec: f64,
    /// Transmit rate in bytes/sec (computed from previous snapshot).
    #[serde(default)]
    pub transmitted_bytes_per_sec: f64,
    pub is_up: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, SchemaRead, SchemaWrite)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStats {
    pub interfaces: Vec<InterfaceStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SchemaRead, SchemaWrite)]
#[serde(rename_all = "camelCase")]
pub struct SystemStats {
    pub os_family: String,
    pub os_manufacturer: String,
    pub os_version: String,
    pub system_manufacturer: String,
    pub system_model: String,
    pub processor_name: String,
    pub uptime_secs: u64,
    pub process_count: u32,
    pub thread_count: u32,
    pub cpu_voltage: f64,
    pub fan_speeds: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SchemaRead, SchemaWrite)]
#[serde(rename_all = "camelCase")]
pub struct FullSystemSnapshot {
    pub timestamp: u64,
    pub cpu: CpuStats,
    pub ram: RamStats,
    pub containers: Vec<ContainerStats>,
    pub storage: StorageStats,
    pub network: NetworkStats,
    pub system: SystemStats,
}
