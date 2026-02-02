// Model serialization tests (JSON camelCase, wincode roundtrip)

use homeserver::models::*;

#[test]
fn test_cpu_stats_serialization_camel_case() {
    let cpu = CpuStats {
        model: "cpu0".into(),
        physical_cores: 4,
        logical_cores: 8,
        usage_percent: 12.5,
        temperature: 45.0,
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
        memory_usage_bytes: 1000,
        memory_limit_bytes: 256 * 1024 * 1024,
        state: "running".into(),
        network_rx_bytes: 0,
        network_tx_bytes: 0,
        block_read_bytes: 0,
        block_write_bytes: 0,
        pids: 10,
        cpu_throttled: false,
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
        },
        ram: RamStats {
            total: 1024,
            used: 512,
            available: 512,
            usage_percent: 50.0,
        },
        containers: vec![],
        storage: StorageStats {
            partitions: vec![],
            disks: vec![],
        },
        network: NetworkStats { interfaces: vec![] },
        system: SystemStats {
            os_family: "Linux".into(),
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
    };
    let json = serde_json::to_string(&snapshot).unwrap();
    assert!(json.contains("\"timestamp\""));
    assert!(json.contains("\"usagePercent\""));
    let back: FullSystemSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(back.timestamp, snapshot.timestamp);
}

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
        },
        ram: RamStats {
            total: 100,
            used: 50,
            available: 50,
            usage_percent: 50.0,
        },
        containers: vec![],
        storage: StorageStats {
            partitions: vec![],
            disks: vec![],
        },
        network: NetworkStats { interfaces: vec![] },
        system: SystemStats {
            os_family: "Linux".into(),
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
    };
    let bytes = wincode::serialize(&snapshot).unwrap();
    let back: FullSystemSnapshot = wincode::deserialize(&bytes).unwrap();
    assert_eq!(back.timestamp, snapshot.timestamp);
    assert_eq!(back.cpu.model, snapshot.cpu.model);
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
        transfer_time_ms: 5,
    };
    let json = serde_json::to_string(&d).unwrap();
    let back: DiskDeviceStat = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, d.name);
    assert_eq!(back.size, d.size);
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
            transfer_time_ms: 0,
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
        is_up: true,
    };
    let json = serde_json::to_string(&i).unwrap();
    let back: InterfaceStat = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, i.name);
    assert_eq!(back.bytes_sent, i.bytes_sent);
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
        cpu_voltage: 1.2,
        fan_speeds: vec![1200, 1400],
    };
    let json = serde_json::to_string(&s).unwrap();
    let _: SystemStats = serde_json::from_str(&json).unwrap();
    let bytes = wincode::serialize(&s).unwrap();
    let back: SystemStats = wincode::deserialize(&bytes).unwrap();
    assert_eq!(back.uptime_secs, s.uptime_secs);
    assert_eq!(back.fan_speeds.len(), 2);
}
