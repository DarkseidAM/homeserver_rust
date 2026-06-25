// Model wincode roundtrip tests (and JSON+wincoding where both apply).

use homeserver::models::*;

#[test]
fn test_full_system_snapshot_wincode_roundtrip() {
    let snapshot = FullSystemSnapshot {
        timestamp: 1,
        cpu: CpuStats {
            model: "m".into(),
            physical_cores: 1,
            logical_cores: 2,
            usage_percent: 0.0,
            temperature: 0.0,
            core_usages: vec![],
        },
        ram: RamStats {
            total: 100,
            used: 50,
            available: 50,
            usage_percent: 50.0,
            swap_total: 0,
            swap_used: 0,
            swap_free: 0,
        },
        containers: vec![],
        storage: StorageStats {
            partitions: vec![],
            disks: vec![],
        },
        network: NetworkStats { interfaces: vec![] },
        system: SystemStatsDynamic {
            uptime_secs: 0,
            process_count: 0,
            thread_count: 0,
            load_avg_1: 0.0,
            load_avg_5: 0.0,
            load_avg_15: 0.0,
        },
        gpus: vec![GpuStats {
            index: 0,
            vendor: "nvidia".into(),
            name: "Test GPU".into(),
            utilization_percent: 42.0,
            memory_used_bytes: 1024,
            memory_total_bytes: 8192,
            temperature_c: 60.0,
            power_watts: Some(120.0),
            fan_percent: Some(35.0),
        }],
        smart: vec![SmartHealth {
            device: "/dev/sda".into(),
            model: "Test SSD".into(),
            health_passed: true,
            temperature_c: Some(40),
            power_on_hours: Some(1234),
            reallocated_sectors: Some(0),
            wear_level_percent: Some(5),
        }],
    };
    let bytes = wincode::serialize(&snapshot).unwrap();
    let back: FullSystemSnapshot = wincode::deserialize(&bytes).unwrap();
    assert_eq!(back.timestamp, snapshot.timestamp);
    assert_eq!(back.cpu.model, snapshot.cpu.model);
    assert_eq!(back.gpus.len(), 1);
    assert_eq!(back.gpus[0].name, "Test GPU");
    assert_eq!(back.gpus[0].power_watts, Some(120.0));
    assert_eq!(back.smart.len(), 1);
    assert_eq!(back.smart[0].device, "/dev/sda");
    assert_eq!(back.smart[0].power_on_hours, Some(1234));
    assert_eq!(back.smart[0].wear_level_percent, Some(5));
}

#[test]
fn test_storage_stats_json_and_wincode_roundtrip() {
    let s = StorageStats {
        partitions: vec![PartitionStat {
            mount: "/data".into(),
            name: "data".into(),
            type_: "xfs".into(),
            total_space: 100,
            used_space: 50,
            available_space: 50,
            usage_percent: 50.0,
        }],
        disks: vec![DiskDeviceStat {
            name: "nvme0".into(),
            model: "NVMe".into(),
            size: 1_000_000_000,
            read_bytes: 0,
            write_bytes: 0,
            io_time_ms: 0,
            iops_read: 0,
            iops_write: 0,
        }],
    };
    let json = serde_json::to_string(&s).unwrap();
    let _: StorageStats = serde_json::from_str(&json).unwrap();
    let bytes = wincode::serialize(&s).unwrap();
    let back: StorageStats = wincode::deserialize(&bytes).unwrap();
    assert_eq!(back.partitions.len(), 1);
    assert_eq!(back.disks.len(), 1);
}

#[test]
fn test_network_stats_json_and_wincode_roundtrip() {
    let n = NetworkStats {
        interfaces: vec![InterfaceStat {
            name: "lo".into(),
            display_name: "loopback".into(),
            mac_address: "".into(),
            ipv4: vec![],
            ipv6: vec!["::1".into()],
            bytes_sent: 0,
            bytes_recv: 0,
            packets_sent: 0,
            packets_recv: 0,
            speed: 0,
            received_bytes_per_sec: 0.0,
            transmitted_bytes_per_sec: 0.0,
            is_up: true,
        }],
    };
    let json = serde_json::to_string(&n).unwrap();
    let _: NetworkStats = serde_json::from_str(&json).unwrap();
    let bytes = wincode::serialize(&n).unwrap();
    let back: NetworkStats = wincode::deserialize(&bytes).unwrap();
    assert_eq!(back.interfaces.len(), 1);
}

#[test]
fn test_system_stats_json_and_wincode_roundtrip() {
    let s = SystemStats {
        os_family: "Linux".into(),
        os_manufacturer: "".into(),
        os_version: "6.0".into(),
        system_manufacturer: "".into(),
        system_model: "PC".into(),
        processor_name: "CPU".into(),
        uptime_secs: 3600,
        process_count: 100,
        thread_count: 200,
        load_avg_1: 0.5,
        load_avg_5: 1.0,
        load_avg_15: 1.5,
    };
    let json = serde_json::to_string(&s).unwrap();
    let _: SystemStats = serde_json::from_str(&json).unwrap();
    let bytes = wincode::serialize(&s).unwrap();
    let back: SystemStats = wincode::deserialize(&bytes).unwrap();
    assert_eq!(back.uptime_secs, s.uptime_secs);
    assert!((back.load_avg_1 - 0.5).abs() < 0.001);
}
