// Docker container models

use serde::{Deserialize, Serialize};
use wincode::{SchemaRead, SchemaWrite};

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
