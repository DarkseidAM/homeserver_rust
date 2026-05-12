// Shared test helpers

use homeserver::models::*;

pub fn minimal_snapshot(timestamp: u64) -> FullSystemSnapshot {
    FullSystemSnapshot {
        timestamp,
        cpu: CpuStats {
            model: String::new(),
            physical_cores: 0,
            logical_cores: 0,
            usage_percent: 0.0,
            temperature: 0.0,
            core_usages: vec![],
        },
        ram: RamStats {
            total: 0,
            used: 0,
            available: 0,
            usage_percent: 0.0,
            swap_total: 0,
            swap_used: 0,
            swap_free: 0,
        },
        containers: vec![],
        storage: StorageStats {
            partitions: vec![],
            disks: vec![],
        },
        network: NetworkStats {
            interfaces: vec![],
        },
        system: SystemStatsDynamic {
            uptime_secs: 0,
            process_count: 0,
            thread_count: 0,
            load_avg_1: 0.0,
            load_avg_5: 0.0,
            load_avg_15: 0.0,
        },
    }
}
