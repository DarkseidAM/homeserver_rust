// CPU, RAM, system identity and snapshot models

use serde::{Deserialize, Serialize};
use wincode::{SchemaRead, SchemaWrite};

use super::{ContainerStats, NetworkStats, StorageStats};

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

/// Static system identity; fetched once at startup and exposed via GET /api/info.
#[derive(Debug, Clone, Serialize, Deserialize, SchemaRead, SchemaWrite)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub os_family: String,
    pub os_manufacturer: String,
    pub os_version: String,
    pub system_manufacturer: String,
    pub system_model: String,
    pub processor_name: String,
}

/// Dynamic-only system metrics (wire + history). Static identity is GET /api/info or WS welcome.
#[derive(Debug, Clone, Serialize, Deserialize, SchemaRead, SchemaWrite)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatsDynamic {
    pub uptime_secs: u64,
    pub process_count: u32,
    pub thread_count: u32,
    pub cpu_voltage: f64,
    pub fan_speeds: Vec<u32>,
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

/// Merge static identity + dynamic metrics for display (e.g. dump_history, legacy readers).
pub fn merge_system_info(info: Option<&SystemInfo>, dynamic: &SystemStatsDynamic) -> SystemStats {
    let (os_family, os_manufacturer, os_version, system_manufacturer, system_model, processor_name) =
        match info {
            Some(i) => (
                i.os_family.clone(),
                i.os_manufacturer.clone(),
                i.os_version.clone(),
                i.system_manufacturer.clone(),
                i.system_model.clone(),
                i.processor_name.clone(),
            ),
            None => (
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ),
        };
    SystemStats {
        os_family,
        os_manufacturer,
        os_version,
        system_manufacturer,
        system_model,
        processor_name,
        uptime_secs: dynamic.uptime_secs,
        process_count: dynamic.process_count,
        thread_count: dynamic.thread_count,
        cpu_voltage: dynamic.cpu_voltage,
        fan_speeds: dynamic.fan_speeds.clone(),
    }
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
    pub system: SystemStatsDynamic,
}

/// Snapshot with merged system (static + dynamic) for display, e.g. dump_history.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullSystemSnapshotDisplay {
    pub timestamp: u64,
    pub cpu: CpuStats,
    pub ram: RamStats,
    pub containers: Vec<ContainerStats>,
    pub storage: StorageStats,
    pub network: NetworkStats,
    pub system: SystemStats,
}
