// Model JSON serde tests (camelCase field names).

use homeserver::models::*;

#[test]
fn test_cpu_stats_serialization_camel_case() {
    let cpu = CpuStats {
        model: "cpu0".into(),
        physical_cores: 4,
        logical_cores: 8,
        usage_percent: 12.5,
        temperature: 45.0,
        core_usages: vec![10.0, 15.0, 20.0, 25.0, 30.0, 35.0, 40.0, 45.0],
    };
    let json = serde_json::to_string(&cpu).unwrap();
    assert!(json.contains("\"usagePercent\""));
    assert!(json.contains("\"physicalCores\""));
    let back: CpuStats = serde_json::from_str(&json).unwrap();
    assert_eq!(back.usage_percent, cpu.usage_percent);
}

#[test]
fn test_ram_stats_json_roundtrip() {
    let ram = RamStats {
        total: 1024,
        used: 512,
        available: 512,
        usage_percent: 50.0,
        swap_total: 2048,
        swap_used: 256,
        swap_free: 1792,
    };
    let json = serde_json::to_string(&ram).unwrap();
    let back: RamStats = serde_json::from_str(&json).unwrap();
    assert_eq!(back.used, ram.used);
}

#[test]
fn test_container_stats_serialization() {
    let c = ContainerStats {
        id: "abc123".into(),
        name: "foo".into(),
        cpu_percent: 1.5,
        cpu_kernel_percent: 0.0,
        cpu_user_percent: 0.0,
        online_cpus: 1,
        memory_usage_bytes: 1000,
        memory_limit_bytes: 256 * 1024 * 1024,
        state: ContainerState::Running,
        network_rx_bytes: 0,
        network_tx_bytes: 0,
        network_rx_packets: 0,
        network_tx_packets: 0,
        network_rx_errors: 0,
        network_tx_errors: 0,
        network_rx_dropped: 0,
        network_tx_dropped: 0,
        block_read_bytes: 0,
        block_write_bytes: 0,
        block_read_ops: 0,
        block_write_ops: 0,
        pids: 10,
        pids_limit: 0,
        cpu_throttled: false,
        cpu_throttled_periods: 0,
        cpu_throttled_time_ns: 0,
        memory_max_usage_bytes: 0,
    };
    let json = serde_json::to_string(&c).unwrap();
    assert!(json.contains("\"memoryUsageBytes\""));
    assert!(json.contains("\"cpuPercent\""));
    let back: ContainerStats = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, c.id);
}

#[test]
fn test_full_system_snapshot_serialization() {
    let snapshot = FullSystemSnapshot {
        timestamp: 12345,
        cpu: CpuStats {
            model: "x".into(),
            physical_cores: 1,
            logical_cores: 2,
            usage_percent: 0.0,
            temperature: 0.0,
            core_usages: vec![],
        },
        ram: RamStats {
            total: 1024,
            used: 512,
            available: 512,
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
        gpus: vec![],
    };
    let json = serde_json::to_string(&snapshot).unwrap();
    assert!(json.contains("\"timestamp\""));
    assert!(json.contains("\"usagePercent\""));
    let back: FullSystemSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(back.timestamp, snapshot.timestamp);
}

#[test]
fn test_partition_stat_json_roundtrip() {
    let p = PartitionStat {
        mount: "/".into(),
        name: "root".into(),
        type_: "ext4".into(),
        total_space: 1000,
        used_space: 400,
        available_space: 600,
        usage_percent: 40.0,
    };
    let json = serde_json::to_string(&p).unwrap();
    let back: PartitionStat = serde_json::from_str(&json).unwrap();
    assert_eq!(back.mount, p.mount);
    assert_eq!(back.usage_percent, p.usage_percent);
}

#[test]
fn test_disk_device_stat_json_roundtrip() {
    let d = DiskDeviceStat {
        name: "sda".into(),
        model: "SSD".into(),
        size: 512 * 1024 * 1024 * 1024,
        read_bytes: 1000,
        write_bytes: 2000,
        io_time_ms: 5,
        iops_read: 100,
        iops_write: 200,
    };
    let json = serde_json::to_string(&d).unwrap();
    let back: DiskDeviceStat = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, d.name);
    assert_eq!(back.size, d.size);
}

#[test]
fn test_interface_stat_json_roundtrip() {
    let i = InterfaceStat {
        name: "eth0".into(),
        display_name: "Ethernet".into(),
        mac_address: "00:11:22:33:44:55".into(),
        ipv4: vec!["192.168.1.1".into()],
        ipv6: vec!["fe80::1".into()],
        bytes_sent: 100,
        bytes_recv: 200,
        packets_sent: 10,
        packets_recv: 20,
        speed: 1000,
        received_bytes_per_sec: 0.0,
        transmitted_bytes_per_sec: 0.0,
        is_up: true,
    };
    let json = serde_json::to_string(&i).unwrap();
    let back: InterfaceStat = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, i.name);
    assert_eq!(back.bytes_sent, i.bytes_sent);
}
