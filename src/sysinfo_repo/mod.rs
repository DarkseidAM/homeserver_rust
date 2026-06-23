// System stats via sysinfo

mod collectors;
pub mod linux;

use crate::models::*;
use std::sync::Arc;
use std::time::Instant;
use sysinfo::{Disks, Networks, System};
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

            let core_usages: Vec<f64> = sys
                .cpus()
                .iter()
                .map(|c| (c.cpu_usage() as f64).clamp(0.0, 100.0))
                .collect();
            let temperature = linux::read_cpu_temperature_linux().unwrap_or(0.0);

            Ok(CpuStats {
                model: name,
                physical_cores: physical,
                logical_cores: logical,
                usage_percent: usage.clamp(0.0, 100.0),
                temperature,
                core_usages,
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
                swap_total: sys.total_swap(),
                swap_used: sys.used_swap(),
                swap_free: sys.free_swap(),
            })
        })
        .await
        .map_err(|e| anyhow::anyhow!("sysinfo task join: {}", e))?
    }
}
