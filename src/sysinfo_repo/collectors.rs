// Secondary impl block for SysinfoRepo: storage, network, system-info and system-stats collectors.

use super::linux;
use crate::models::*;
use std::time::Instant;
use sysinfo::{ProcessesToUpdate, System};
use tracing::instrument;

use super::SysinfoRepo;

impl SysinfoRepo {
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

            let diskstats = linux::read_diskstats_linux();
            let disk_devices: Vec<DiskDeviceStat> = disks_guard
                .list()
                .iter()
                .map(|d| {
                    let raw_name: String = d.name().to_string_lossy().into_owned();
                    // Strip /dev/ prefix so "sda" matches diskstats key "sda"
                    let dev_name = raw_name.trim_start_matches("/dev/").to_string();
                    let io = diskstats.get(&dev_name).cloned().unwrap_or_default();
                    DiskDeviceStat {
                        name: raw_name,
                        model: linux::read_disk_model_linux(&dev_name),
                        size: d.total_space(),
                        read_bytes: io.sectors_read * 512,
                        write_bytes: io.sectors_written * 512,
                        io_time_ms: io.io_time_ms,
                        iops_read: io.reads_completed,
                        iops_write: io.writes_completed,
                    }
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
                    is_up: linux::read_interface_operstate(name),
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
            let (load_avg_1, load_avg_5, load_avg_15) =
                linux::read_loadavg_linux().unwrap_or((0.0, 0.0, 0.0));
            Ok(SystemStatsDynamic {
                uptime_secs: uptime,
                process_count,
                thread_count,
                load_avg_1,
                load_avg_5,
                load_avg_15,
            })
        })
        .await
        .map_err(|e| anyhow::anyhow!("sysinfo task join: {}", e))?
    }
}
