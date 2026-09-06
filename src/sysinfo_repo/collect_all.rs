// Combined collector for sysinfo metrics inside a single spawn_blocking dispatch.

use super::collectors::{collect_network_inner, collect_storage_inner};
use super::system_stats::collect_system_dynamic_inner;
use super::{SysinfoRepo, collect_cpu_inner, collect_ram_inner};
use crate::models::*;
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct SysinfoSnapshot {
    pub cpu: CpuStats,
    pub ram: RamStats,
    pub storage: StorageStats,
    pub network: NetworkStats,
    pub system: SystemStatsDynamic,
}

impl SysinfoRepo {
    #[instrument(skip(self), fields(repo = "sysinfo", operation = "collect_all"))]
    pub async fn collect_all(&self) -> SysinfoSnapshot {
        let sys = self.sys.clone();
        let disks = self.disks.clone();
        let networks = self.networks.clone();
        let last_network = self.last_network.clone();
        let last_cpu_refresh = self.last_cpu_refresh.clone();
        let cpu_model = self.cpu_model.clone();
        let physical_cores = self.physical_cores;
        let hwmon_temp_path = self.hwmon_temp_path.clone();
        let iface_speeds = self.iface_speeds.clone();
        let disk_models = self.disk_models.clone();

        tokio::task::spawn_blocking(move || {
            // 1. CPU and RAM collected under single sys lock
            let (cpu, ram) = match sys.lock() {
                Ok(mut sys_guard) => {
                    let cpu = collect_cpu_inner(
                        &mut sys_guard,
                        &last_cpu_refresh,
                        &cpu_model,
                        physical_cores,
                        hwmon_temp_path.as_deref(),
                    );
                    let ram = collect_ram_inner(&mut sys_guard);
                    (cpu, ram)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "sysinfo sys lock poisoned");
                    (CpuStats::default(), RamStats::default())
                }
            };

            // 2. Storage
            let storage = collect_storage_inner(&disks, &disk_models);

            // 3. Network
            let network = collect_network_inner(&networks, &last_network, &iface_speeds);

            // 4. System dynamic stats
            let system = collect_system_dynamic_inner(&sys);

            SysinfoSnapshot {
                cpu,
                ram,
                storage,
                network,
                system,
            }
        })
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "sysinfo collect_all task join failed; using defaults");
            SysinfoSnapshot {
                cpu: CpuStats::default(),
                ram: RamStats::default(),
                storage: StorageStats::default(),
                network: NetworkStats::default(),
                system: SystemStatsDynamic::default(),
            }
        })
    }
}
