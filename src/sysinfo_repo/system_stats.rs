// System info (static identity) and system stats (dynamic load/proc/uptime).

use super::SysinfoRepo;
use super::linux;
use crate::models::*;
use std::sync::Mutex;
use sysinfo::{ProcessesToUpdate, System};
use tracing::instrument;

pub(super) fn collect_system_dynamic_inner(sys: &Mutex<System>) -> SystemStatsDynamic {
    let uptime = System::uptime();
    let (load_avg_1, load_avg_5, load_avg_15) =
        linux::read_loadavg_linux().unwrap_or((0.0, 0.0, 0.0));

    let (process_count, thread_count) = match linux::read_proc_entity_counts() {
        Some(counts) => counts,
        None => {
            if let Ok(mut sys_guard) = sys.lock() {
                sys_guard.refresh_processes(ProcessesToUpdate::All, true);
                let process_count = sys_guard.processes().len() as u32;
                let thread_count = sys_guard
                    .processes()
                    .values()
                    .map(|p| 1 + p.tasks().map(|t| t.len()).unwrap_or(0))
                    .sum::<usize>()
                    .min(u32::MAX as usize) as u32;
                (process_count, thread_count)
            } else {
                (0, 0)
            }
        }
    };

    SystemStatsDynamic {
        uptime_secs: uptime,
        process_count,
        thread_count,
        load_avg_1,
        load_avg_5,
        load_avg_15,
    }
}

impl SysinfoRepo {
    #[instrument(skip(self), fields(repo = "sysinfo", operation = "get_system_info"))]
    pub async fn get_system_info(&self) -> anyhow::Result<SystemInfo> {
        let sys = self.sys.clone();
        let cpu_model = self.cpu_model.clone();
        tokio::task::spawn_blocking(move || {
            let sys = sys
                .lock()
                .map_err(|e| anyhow::anyhow!("sysinfo lock poisoned: {}", e))?;
            let name = System::name().unwrap_or_else(|| std::env::consts::OS.into());
            let os_version = System::os_version().unwrap_or_default();
            let host_name = System::host_name().unwrap_or_default();
            let cpu_name = if !cpu_model.is_empty() && cpu_model != "Unknown" {
                cpu_model
            } else {
                sys.cpus()
                    .first()
                    .map(|c| c.name().to_string())
                    .filter(|s| !s.is_empty() && s != "cpu0")
                    .unwrap_or_else(|| "Unknown".into())
            };
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
        tokio::task::spawn_blocking(move || Ok(collect_system_dynamic_inner(&sys)))
            .await
            .map_err(|e| anyhow::anyhow!("sysinfo task join: {}", e))?
    }
}
