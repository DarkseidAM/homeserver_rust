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
        },
        ram: RamStats {
            total: 0,
            used: 0,
            available: 0,
            usage_percent: 0.0,
        },
        containers: vec![],
        storage: StorageStats {
            partitions: vec![],
            disks: vec![],
        },
        network: NetworkStats {
            interfaces: vec![],
        },
        system: SystemStats {
            os_family: String::new(),
            os_manufacturer: String::new(),
            os_version: String::new(),
            system_manufacturer: String::new(),
            system_model: String::new(),
            processor_name: String::new(),
            uptime_secs: 0,
            process_count: 0,
            thread_count: 0,
            cpu_voltage: 0.0,
            fan_speeds: vec![],
        },
    }
}
