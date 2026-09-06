// Storage and network stats collectors with capacity hints and read-through caching.

use super::linux;
use super::{NetworkCounters, SysinfoRepo};
use crate::models::*;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use sysinfo::{Disks, Networks};
use tracing::instrument;

/// Times we skipped a per-interface rate sample because a cumulative counter decreased (reset / driver quirk).
static NETWORK_RATE_COUNTER_DECREASE_SKIPS: AtomicU64 = AtomicU64::new(0);

pub(super) fn log_counter_decrease(iface: &str, field: &str, curr: u64, prev: u64) {
    let n = NETWORK_RATE_COUNTER_DECREASE_SKIPS.fetch_add(1, Ordering::Relaxed) + 1;
    tracing::debug!(
        operation = "network_rate_skip",
        iface = %iface,
        field = field,
        curr = curr,
        prev = prev,
        skips_total = n,
        "cumulative {} counter decreased; skipping rate this interval",
        field
    );
}

pub(super) fn collect_storage_inner(
    disks: &Mutex<Disks>,
    disk_models: &Mutex<HashMap<String, String>>,
) -> StorageStats {
    let mut disks_guard = match disks.lock() {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!(error = %e, "sysinfo disks lock poisoned");
            return StorageStats::default();
        }
    };

    disks_guard.refresh(true);
    let disk_count = disks_guard.list().len();
    let mut partitions: Vec<PartitionStat> = Vec::with_capacity(disk_count);
    for d in disks_guard.list() {
        let total = d.total_space();
        let available = d.available_space();
        let used = total.saturating_sub(available);
        let usage_percent = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        partitions.push(PartitionStat {
            mount: d.mount_point().to_string_lossy().into_owned(),
            name: d.name().to_string_lossy().into_owned(),
            type_: d.file_system().to_string_lossy().into_owned(),
            total_space: total,
            used_space: used,
            available_space: available,
            usage_percent,
        });
    }

    let diskstats = linux::read_diskstats_linux();
    let mut disk_devices: Vec<DiskDeviceStat> = Vec::with_capacity(disk_count);
    let mut cached_models = disk_models.lock().ok();

    for d in disks_guard.list() {
        let raw_name: String = d.name().to_string_lossy().into_owned();
        let dev_name = raw_name.trim_start_matches("/dev/").to_string();
        let io = diskstats.get(&dev_name).cloned().unwrap_or_default();
        let model = if let Some(ref mut map) = cached_models {
            if let Some(m) = map.get(&dev_name) {
                m.clone()
            } else {
                let m = linux::read_disk_model_linux(&dev_name);
                map.insert(dev_name.clone(), m.clone());
                m
            }
        } else {
            linux::read_disk_model_linux(&dev_name)
        };
        disk_devices.push(DiskDeviceStat {
            name: raw_name,
            model,
            size: d.total_space(),
            read_bytes: io.sectors_read * 512,
            write_bytes: io.sectors_written * 512,
            io_time_ms: io.io_time_ms,
            iops_read: io.reads_completed,
            iops_write: io.writes_completed,
        });
    }

    StorageStats {
        partitions,
        disks: disk_devices,
    }
}

pub(super) fn collect_network_inner(
    networks: &Mutex<Networks>,
    last_network: &Mutex<Option<(HashMap<String, NetworkCounters>, Instant)>>,
    iface_speeds: &Mutex<HashMap<String, u64>>,
) -> NetworkStats {
    let mut networks_guard = match networks.lock() {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!(error = %e, "sysinfo networks lock poisoned");
            return NetworkStats::default();
        }
    };

    networks_guard.refresh(true);
    let iface_count = networks_guard.list().len();
    let mut interfaces: Vec<InterfaceStat> = Vec::with_capacity(iface_count);
    let mut cached_speeds = iface_speeds.lock().ok();

    for (name, data) in networks_guard.list() {
        let speed = if let Some(ref mut map) = cached_speeds {
            if let Some(&s) = map.get(name) {
                s
            } else {
                let s = linux::get_interface_speed(name);
                map.insert(name.clone(), s);
                s
            }
        } else {
            linux::get_interface_speed(name)
        };

        interfaces.push(InterfaceStat {
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
            speed,
            received_bytes_per_sec: 0.0,
            transmitted_bytes_per_sec: 0.0,
            is_up: linux::read_interface_operstate(name),
        });
    }

    let now = Instant::now();
    if let Ok(mut last_guard) = last_network.lock() {
        if let Some((ref prev_map, prev_ts)) = *last_guard {
            let dt_secs = now.duration_since(prev_ts).as_secs_f64();
            if dt_secs > 0.0 {
                for iface in &mut interfaces {
                    if let Some(p) = prev_map.get(&iface.name) {
                        if iface.bytes_recv >= p.bytes_recv {
                            let drx = iface.bytes_recv - p.bytes_recv;
                            iface.received_bytes_per_sec = drx as f64 / dt_secs;
                        } else {
                            log_counter_decrease(
                                &iface.name,
                                "bytes_recv",
                                iface.bytes_recv,
                                p.bytes_recv,
                            );
                        }
                        if iface.bytes_sent >= p.bytes_sent {
                            let dtx = iface.bytes_sent - p.bytes_sent;
                            iface.transmitted_bytes_per_sec = dtx as f64 / dt_secs;
                        } else {
                            log_counter_decrease(
                                &iface.name,
                                "bytes_sent",
                                iface.bytes_sent,
                                p.bytes_sent,
                            );
                        }
                    }
                }
            }
        }
        let mut new_map = HashMap::with_capacity(interfaces.len());
        for iface in &interfaces {
            new_map.insert(
                iface.name.clone(),
                NetworkCounters {
                    bytes_recv: iface.bytes_recv,
                    bytes_sent: iface.bytes_sent,
                },
            );
        }
        *last_guard = Some((new_map, now));
    }

    NetworkStats { interfaces }
}

impl SysinfoRepo {
    #[instrument(skip(self), fields(repo = "sysinfo", operation = "get_storage_stats"))]
    pub async fn get_storage_stats(&self) -> anyhow::Result<StorageStats> {
        let disks = self.disks.clone();
        let disk_models = self.disk_models.clone();
        tokio::task::spawn_blocking(move || Ok(collect_storage_inner(&disks, &disk_models)))
            .await
            .map_err(|e| anyhow::anyhow!("sysinfo task join: {}", e))?
    }

    #[instrument(skip(self), fields(repo = "sysinfo", operation = "get_network_stats"))]
    pub async fn get_network_stats(&self) -> anyhow::Result<NetworkStats> {
        let networks = self.networks.clone();
        let last_network = self.last_network.clone();
        let iface_speeds = self.iface_speeds.clone();
        tokio::task::spawn_blocking(move || {
            Ok(collect_network_inner(
                &networks,
                &last_network,
                &iface_speeds,
            ))
        })
        .await
        .map_err(|e| anyhow::anyhow!("sysinfo task join: {}", e))?
    }
}
