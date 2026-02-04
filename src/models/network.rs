// Network interface models

use serde::{Deserialize, Serialize};
use wincode::{SchemaRead, SchemaWrite};

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
    #[serde(default)]
    pub received_bytes_per_sec: f64,
    #[serde(default)]
    pub transmitted_bytes_per_sec: f64,
    pub is_up: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, SchemaRead, SchemaWrite)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStats {
    pub interfaces: Vec<InterfaceStat>,
}
