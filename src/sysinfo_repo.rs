// System stats via sysinfo (OSHI equivalent)

use crate::models::*;
use std::sync::Arc;
use sysinfo::{Disks, Networks, System};

pub struct SysinfoRepo {
    sys: Arc<std::sync::Mutex<System>>,
    disks: Arc<std::sync::Mutex<Disks>>,
    networks: Arc<std::sync::Mutex<Networks>>,
}

impl Default for SysinfoRepo {
    fn default() -> Self {
        Self::new()
    }
}

impl SysinfoRepo {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();
        Self {
            sys: Arc::new(std::sync::Mutex::new(sys)),
            disks: Arc::new(std::sync::Mutex::new(disks)),
            networks: Arc::new(std::sync::Mutex::new(networks)),
        }
    }

    pub async fn get_cpu_stats(&self) -> anyhow::Result<CpuStats> {
        let sys = self.sys.clone();
        tokio::task::spawn_blocking(move || {
            let mut sys = sys
                .lock()
                .map_err(|e| anyhow::anyhow!("sysinfo lock poisoned: {}", e))?;
            sys.refresh_cpu_all();
            std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
            sys.refresh_cpu_all();

            let usage = sys.global_cpu_usage() as f64;
            let physical = System::physical_core_count().unwrap_or(0) as u32;
            let logical = sys.cpus().len() as u32;
            let name = sys
                .cpus()
                .first()
                .map(|c| c.name().to_string())
                .unwrap_or_else(|| "Unknown".into());

            Ok(CpuStats {
                model: name,
                physical_cores: physical,
                logical_cores: logical,
                usage_percent: usage.clamp(0.0, 100.0),
                temperature: 0.0,
            })
        })
        .await
        .map_err(|e| anyhow::anyhow!("sysinfo task join: {}", e))?
    }

    pub async fn get_ram_stats(&self) -> anyhow::Result<RamStats> {
        let sys = self.sys.clone();
        tokio::task::spawn_blocking(move || {
            let mut sys = sys
                .lock()
                .map_err(|e| anyhow::anyhow!("sysinfo lock poisoned: {}", e))?;
            sys.refresh_memory();

            let total = sys.total_memory();
            let available = sys.available_memory();
            let used = total.saturating_sub(available);
            let usage_percent = if total > 0 {
                (used as f64 / total as f64) * 100.0
            } else {
                0.0
            };

            Ok(RamStats {
                total,
                used,
                available,
                usage_percent,
            })
        })
        .await
        .map_err(|e| anyhow::anyhow!("sysinfo task join: {}", e))?
    }

    pub async fn get_storage_stats(&self) -> anyhow::Result<StorageStats> {
        let disks = self.disks.clone();
        tokio::task::spawn_blocking(move || {
            let mut disks_guard = disks
                .lock()
                .map_err(|e| anyhow::anyhow!("sysinfo disks lock poisoned: {}", e))?;
            disks_guard.refresh(false);
            let partitions: Vec<PartitionStat> = disks_guard
                .list()
                .iter()
                .map(|d| {
                    let total = d.total_space();
                    let available = d.available_space();
                    let used = total.saturating_sub(available);
                    let usage_percent = if total > 0 {
                        (used as f64 / total as f64) * 100.0
                    } else {
                        0.0
                    };
                    PartitionStat {
                        mount: d.mount_point().to_string_lossy().into_owned(),
                        name: d.name().to_string_lossy().into_owned(),
                        type_: d.file_system().to_string_lossy().into_owned(),
                        total_space: total,
                        used_space: used,
                        available_space: available,
                        usage_percent,
                    }
                })
                .collect();

            let disk_devices: Vec<DiskDeviceStat> = disks_guard
                .list()
                .iter()
                .map(|d| DiskDeviceStat {
                    name: d.name().to_string_lossy().into_owned(),
                    model: String::new(),
                    size: d.total_space(),
                    read_bytes: 0,
                    write_bytes: 0,
                    transfer_time_ms: 0,
                })
                .collect();

            Ok(StorageStats {
                partitions,
                disks: disk_devices,
            })
        })
        .await
        .map_err(|e| anyhow::anyhow!("sysinfo task join: {}", e))?
    }

    pub async fn get_network_stats(&self) -> anyhow::Result<NetworkStats> {
        let networks = self.networks.clone();
        tokio::task::spawn_blocking(move || {
            let mut networks_guard = networks
                .lock()
                .map_err(|e| anyhow::anyhow!("sysinfo networks lock poisoned: {}", e))?;
            networks_guard.refresh(true);
            let interfaces: Vec<InterfaceStat> = networks_guard
                .list()
                .iter()
                .map(|(name, data)| InterfaceStat {
                    name: name.clone(),
                    display_name: name.clone(),
                    mac_address: data.mac_address().to_string(),
                    ipv4: data
                        .ip_networks()
                        .iter()
                        .filter(|n| n.addr.is_ipv4())
                        .map(|n| n.addr.to_string())
                        .collect(),
                    ipv6: data
                        .ip_networks()
                        .iter()
                        .filter(|n| n.addr.is_ipv6())
                        .map(|n| n.addr.to_string())
                        .collect(),
                    bytes_sent: data.transmitted(),
                    bytes_recv: data.received(),
                    packets_sent: data.packets_transmitted(),
                    packets_recv: data.packets_received(),
                    speed: 0,
                    is_up: true,
                })
                .collect();

            Ok(NetworkStats { interfaces })
        })
        .await
        .map_err(|e| anyhow::anyhow!("sysinfo task join: {}", e))?
    }

    pub async fn get_system_stats(&self) -> anyhow::Result<SystemStats> {
        let sys = self.sys.clone();
        tokio::task::spawn_blocking(move || {
            let mut sys = sys
                .lock()
                .map_err(|e| anyhow::anyhow!("sysinfo lock poisoned: {}", e))?;
            sys.refresh_memory();
            sys.refresh_cpu_all();

            let name = System::name().unwrap_or_else(|| std::env::consts::OS.into());
            let os_version = System::os_version().unwrap_or_default();
            let host_name = System::host_name().unwrap_or_default();
            let cpu_name = sys
                .cpus()
                .first()
                .map(|c| c.name().to_string())
                .unwrap_or_else(|| "Unknown".into());
            let uptime = System::uptime();
            let process_count = sys.processes().len() as u32;

            Ok(SystemStats {
                os_family: name,
                os_manufacturer: String::new(),
                os_version,
                system_manufacturer: String::new(),
                system_model: host_name,
                processor_name: cpu_name,
                uptime_secs: uptime,
                process_count,
                thread_count: 0,
                cpu_voltage: 0.0,
                fan_speeds: vec![],
            })
        })
        .await
        .map_err(|e| anyhow::anyhow!("sysinfo task join: {}", e))?
    }
}
