// System stats via sysinfo

mod linux;

use crate::models::*;
use std::sync::Arc;
use std::time::Instant;
use sysinfo::{Disks, Networks, ProcessesToUpdate, System};
use tracing::instrument;

pub struct SysinfoRepo {
    sys: Arc<std::sync::Mutex<System>>,
    disks: Arc<std::sync::Mutex<Disks>>,
    networks: Arc<std::sync::Mutex<Networks>>,
    last_network: Arc<std::sync::Mutex<Option<(NetworkStats, Instant)>>>,
    last_cpu_refresh: Arc<std::sync::Mutex<Option<(Instant, f64)>>>,
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
            last_network: Arc::new(std::sync::Mutex::new(None)),
            last_cpu_refresh: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    #[instrument(skip(self), fields(repo = "sysinfo", operation = "get_cpu_stats"))]
    pub async fn get_cpu_stats(&self) -> anyhow::Result<CpuStats> {
        let sys = self.sys.clone();
        let last_cpu_refresh = self.last_cpu_refresh.clone();
        tokio::task::spawn_blocking(move || {
            let mut sys = sys
                .lock()
                .map_err(|e| anyhow::anyhow!("sysinfo lock poisoned: {}", e))?;

            let now = Instant::now();
            let usage = if let Ok(mut guard) = last_cpu_refresh.lock() {
                if let Some((prev_ts, prev_usage)) = *guard {
                    let dt = now.duration_since(prev_ts);
                    if dt >= sysinfo::MINIMUM_CPU_UPDATE_INTERVAL {
                        // Enough time has passed, refresh and get new usage
                        sys.refresh_cpu_all();
                        let new_usage = sys.global_cpu_usage() as f64;
                        *guard = Some((now, new_usage));
                        new_usage
                    } else {
                        // Not enough time has passed, return cached usage without blocking
                        prev_usage
                    }
                } else {
                    // First call: refresh to establish baseline
                    sys.refresh_cpu_all();
                    *guard = Some((now, 0.0));
                    0.0
                }
            } else {
                // Lock failed, refresh and return 0.0
                sys.refresh_cpu_all();
                0.0
            };

            let physical = System::physical_core_count().unwrap_or(0) as u32;
            let logical = sys.cpus().len() as u32;
            let name = linux::read_cpu_model_linux()
                .or_else(|| {
                    sys.cpus()
                        .first()
                        .map(|c| c.name().to_string())
                        .filter(|s| !s.is_empty() && s != "cpu0")
                })
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

    #[instrument(skip(self), fields(repo = "sysinfo", operation = "get_ram_stats"))]
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

    #[instrument(skip(self), fields(repo = "sysinfo", operation = "get_storage_stats"))]
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

    #[instrument(skip(self), fields(repo = "sysinfo", operation = "get_network_stats"))]
    pub async fn get_network_stats(&self) -> anyhow::Result<NetworkStats> {
        let networks = self.networks.clone();
        let last_network = self.last_network.clone();
        tokio::task::spawn_blocking(move || {
            let mut networks_guard = networks
                .lock()
                .map_err(|e| anyhow::anyhow!("sysinfo networks lock poisoned: {}", e))?;
            networks_guard.refresh(true);
            let mut interfaces: Vec<InterfaceStat> = networks_guard
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
                    speed: linux::get_interface_speed(name),
                    received_bytes_per_sec: 0.0,
                    transmitted_bytes_per_sec: 0.0,
                    is_up: true,
                })
                .collect();

            let now = Instant::now();
            if let Ok(mut guard) = last_network.lock() {
                if let Some((ref prev, prev_ts)) = *guard {
                    let dt_secs = now.duration_since(prev_ts).as_secs_f64();
                    if dt_secs > 0.0 {
                        for iface in &mut interfaces {
                            if let Some(p) = prev.interfaces.iter().find(|i| i.name == iface.name) {
                                let drx = iface.bytes_recv.saturating_sub(p.bytes_recv);
                                let dtx = iface.bytes_sent.saturating_sub(p.bytes_sent);
                                iface.received_bytes_per_sec = drx as f64 / dt_secs;
                                iface.transmitted_bytes_per_sec = dtx as f64 / dt_secs;
                            }
                        }
                    }
                }
                *guard = Some((
                    NetworkStats {
                        interfaces: interfaces.clone(),
                    },
                    now,
                ));
            }

            Ok(NetworkStats { interfaces })
        })
        .await
        .map_err(|e| anyhow::anyhow!("sysinfo task join: {}", e))?
    }

    #[instrument(skip(self), fields(repo = "sysinfo", operation = "get_system_info"))]
    pub async fn get_system_info(&self) -> anyhow::Result<SystemInfo> {
        let sys = self.sys.clone();
        tokio::task::spawn_blocking(move || {
            let sys = sys
                .lock()
                .map_err(|e| anyhow::anyhow!("sysinfo lock poisoned: {}", e))?;
            let name = System::name().unwrap_or_else(|| std::env::consts::OS.into());
            let os_version = System::os_version().unwrap_or_default();
            let host_name = System::host_name().unwrap_or_default();
            let cpu_name = linux::read_cpu_model_linux()
                .or_else(|| {
                    sys.cpus()
                        .first()
                        .map(|c| c.name().to_string())
                        .filter(|s| !s.is_empty() && s != "cpu0")
                })
                .unwrap_or_else(|| "Unknown".into());
            let os_manufacturer = linux::read_os_manufacturer_linux().unwrap_or_default();
            let system_manufacturer = linux::read_sys_vendor_linux().unwrap_or_default();
            Ok(SystemInfo {
                os_family: name,
                os_manufacturer,
                os_version,
                system_manufacturer,
                system_model: host_name,
                processor_name: cpu_name,
            })
        })
        .await
        .map_err(|e| anyhow::anyhow!("sysinfo task join: {}", e))?
    }

    /// Returns dynamic-only system metrics (wire format). Static identity is GET /api/info.
    #[instrument(skip(self), fields(repo = "sysinfo", operation = "get_system_stats"))]
    pub async fn get_system_stats(&self) -> anyhow::Result<SystemStatsDynamic> {
        let sys = self.sys.clone();
        tokio::task::spawn_blocking(move || {
            let mut sys = sys
                .lock()
                .map_err(|e| anyhow::anyhow!("sysinfo lock poisoned: {}", e))?;
            sys.refresh_memory();
            sys.refresh_cpu_all();
            sys.refresh_processes(ProcessesToUpdate::All, true);
            let uptime = System::uptime();
            let process_count = sys.processes().len() as u32;
            let thread_count = sys
                .processes()
                .values()
                .map(|p| 1 + p.tasks().map(|t| t.len()).unwrap_or(0))
                .sum::<usize>()
                .min(u32::MAX as usize) as u32;
            Ok(SystemStatsDynamic {
                uptime_secs: uptime,
                process_count,
                thread_count,
                cpu_voltage: 0.0,
                fan_speeds: vec![],
            })
        })
        .await
        .map_err(|e| anyhow::anyhow!("sysinfo task join: {}", e))?
    }
}
