// System stats via sysinfo

mod collect_all;
mod collectors;
pub mod linux;
mod system_stats;

pub use collect_all::SysinfoSnapshot;

use crate::models::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use sysinfo::{Disks, Networks, System};
use tracing::instrument;

#[derive(Debug, Clone, Default)]
pub struct NetworkCounters {
    pub bytes_recv: u64,
    pub bytes_sent: u64,
}

pub(super) type LastNetworkSample = Option<(HashMap<String, NetworkCounters>, Instant)>;

pub struct SysinfoRepo {
    pub(super) sys: Arc<Mutex<System>>,
    pub(super) disks: Arc<Mutex<Disks>>,
    pub(super) networks: Arc<Mutex<Networks>>,
    pub(super) last_network: Arc<Mutex<LastNetworkSample>>,
    pub(super) last_cpu_refresh: Arc<Mutex<Option<(Instant, f64)>>>,
    pub(super) cpu_model: String,
    pub(super) physical_cores: u32,
    pub(super) hwmon_temp_path: Option<PathBuf>,
    pub(super) iface_speeds: Arc<Mutex<HashMap<String, u64>>>,
    pub(super) disk_models: Arc<Mutex<HashMap<String, String>>>,
}

impl Default for SysinfoRepo {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) fn collect_cpu_inner(
    sys: &mut System,
    last_cpu_refresh: &Mutex<Option<(Instant, f64)>>,
    cpu_model: &str,
    physical_cores: u32,
    hwmon_temp_path: Option<&std::path::Path>,
) -> CpuStats {
    let now = Instant::now();
    let usage = if let Ok(mut guard) = last_cpu_refresh.lock() {
        if let Some((prev_ts, prev_usage)) = *guard {
            let dt = now.duration_since(prev_ts);
            if dt >= sysinfo::MINIMUM_CPU_UPDATE_INTERVAL {
                sys.refresh_cpu_all();
                let new_usage = sys.global_cpu_usage() as f64;
                *guard = Some((now, new_usage));
                new_usage
            } else {
                prev_usage
            }
        } else {
            sys.refresh_cpu_all();
            *guard = Some((now, 0.0));
            0.0
        }
    } else {
        sys.refresh_cpu_all();
        0.0
    };

    let logical = sys.cpus().len() as u32;
    let core_usages: Vec<f64> = sys
        .cpus()
        .iter()
        .map(|c| (c.cpu_usage() as f64).clamp(0.0, 100.0))
        .collect();
    let temperature = hwmon_temp_path
        .and_then(linux::read_cpu_temperature_from_path)
        .unwrap_or(0.0);

    CpuStats {
        model: cpu_model.to_string(),
        physical_cores,
        logical_cores: logical,
        usage_percent: usage.clamp(0.0, 100.0),
        temperature,
        core_usages,
    }
}

pub(super) fn collect_ram_inner(sys: &mut System) -> RamStats {
    sys.refresh_memory();

    let total = sys.total_memory();
    let available = sys.available_memory();
    let used = total.saturating_sub(available);
    let usage_percent = if total > 0 {
        (used as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    RamStats {
        total,
        used,
        available,
        usage_percent,
        swap_total: sys.total_swap(),
        swap_used: sys.used_swap(),
        swap_free: sys.free_swap(),
    }
}

impl SysinfoRepo {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();

        let cpu_model = linux::read_cpu_model_linux()
            .or_else(|| {
                sys.cpus()
                    .first()
                    .map(|c| c.name().to_string())
                    .filter(|s| !s.is_empty() && s != "cpu0")
            })
            .unwrap_or_else(|| "Unknown".into());
        let physical_cores = System::physical_core_count().unwrap_or(0) as u32;
        let hwmon_temp_path = linux::find_cpu_temperature_path_linux();

        let mut iface_speeds = HashMap::new();
        for name in networks.list().keys() {
            let speed = linux::get_interface_speed(name);
            iface_speeds.insert(name.clone(), speed);
        }

        let mut disk_models = HashMap::new();
        for d in disks.list() {
            let raw_name: String = d.name().to_string_lossy().into_owned();
            let dev_name = raw_name.trim_start_matches("/dev/");
            let model = linux::read_disk_model_linux(dev_name);
            disk_models.insert(dev_name.to_string(), model);
        }

        Self {
            sys: Arc::new(Mutex::new(sys)),
            disks: Arc::new(Mutex::new(disks)),
            networks: Arc::new(Mutex::new(networks)),
            last_network: Arc::new(Mutex::new(None)),
            last_cpu_refresh: Arc::new(Mutex::new(None)),
            cpu_model,
            physical_cores,
            hwmon_temp_path,
            iface_speeds: Arc::new(Mutex::new(iface_speeds)),
            disk_models: Arc::new(Mutex::new(disk_models)),
        }
    }

    #[instrument(skip(self), fields(repo = "sysinfo", operation = "get_cpu_stats"))]
    pub async fn get_cpu_stats(&self) -> anyhow::Result<CpuStats> {
        let sys = self.sys.clone();
        let last_cpu_refresh = self.last_cpu_refresh.clone();
        let cpu_model = self.cpu_model.clone();
        let physical_cores = self.physical_cores;
        let hwmon_temp_path = self.hwmon_temp_path.clone();
        tokio::task::spawn_blocking(move || {
            let mut sys = sys
                .lock()
                .map_err(|e| anyhow::anyhow!("sysinfo lock poisoned: {}", e))?;
            Ok(collect_cpu_inner(
                &mut sys,
                &last_cpu_refresh,
                &cpu_model,
                physical_cores,
                hwmon_temp_path.as_deref(),
            ))
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
            Ok(collect_ram_inner(&mut sys))
        })
        .await
        .map_err(|e| anyhow::anyhow!("sysinfo task join: {}", e))?
    }
}
