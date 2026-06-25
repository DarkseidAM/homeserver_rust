// Aggregation logic tests: aggregate_snapshots (avg/min/max, container aggregation)

use homeserver::history_repo::aggregation::aggregate_snapshots;
use homeserver::models::*;

fn snapshot(ts: u64, cpu_percent: f64, memory_used: u64) -> FullSystemSnapshot {
    FullSystemSnapshot {
        timestamp: ts,
        cpu: CpuStats {
            model: String::new(),
            physical_cores: 0,
            logical_cores: 0,
            usage_percent: cpu_percent,
            temperature: 0.0,
            core_usages: vec![],
        },
        ram: RamStats {
            total: 0,
            used: memory_used,
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
        smart: vec![],
    }
}

#[test]
fn aggregate_snapshots_empty_returns_none() {
    let snapshots: Vec<FullSystemSnapshot> = vec![];
    let out = aggregate_snapshots(&snapshots, 60_000, 60);
    assert!(out.is_none());
}

#[test]
fn aggregate_snapshots_single_snapshot() {
    let snapshots = vec![snapshot(60_000, 25.0, 512)];
    let out = aggregate_snapshots(&snapshots, 60_000, 60).unwrap();
    assert_eq!(out.created_at, 60_000);
    assert_eq!(out.resolution_seconds, 60);
    assert_eq!(out.cpu_load_avg, 25.0);
    assert_eq!(out.cpu_load_min, 25.0);
    assert_eq!(out.cpu_load_max, 25.0);
    assert_eq!(out.memory_used_avg, 512);
    assert_eq!(out.memory_used_min, 512);
    assert_eq!(out.memory_used_max, 512);
}

#[test]
fn aggregate_snapshots_multiple_computes_avg_min_max() {
    let snapshots = vec![
        snapshot(60_000, 10.0, 100),
        snapshot(60_001, 20.0, 200),
        snapshot(60_002, 30.0, 300),
    ];
    let out = aggregate_snapshots(&snapshots, 60_000, 60).unwrap();
    assert_eq!(out.created_at, 60_000);
    assert_eq!(out.cpu_load_avg, 20.0);
    assert_eq!(out.cpu_load_min, 10.0);
    assert_eq!(out.cpu_load_max, 30.0);
    assert_eq!(out.memory_used_avg, 200);
    assert_eq!(out.memory_used_min, 100);
    assert_eq!(out.memory_used_max, 300);
}

#[test]
fn aggregate_aggregated_snapshots_empty_returns_none() {
    let aggs: Vec<homeserver::models::AggregatedSnapshot> = vec![];
    let out =
        homeserver::history_repo::aggregation::aggregate_aggregated_snapshots(&aggs, 300_000, 300);
    assert!(out.is_none());
}

#[test]
fn aggregate_aggregated_snapshots_five_one_min_produces_5min() {
    let one_min =
        |created_at: i64, cpu_avg: f64, mem_avg: i64| homeserver::models::AggregatedSnapshot {
            created_at,
            resolution_seconds: 60,
            cpu_load_avg: cpu_avg,
            cpu_load_min: cpu_avg - 1.0,
            cpu_load_max: cpu_avg + 1.0,
            memory_used_avg: mem_avg,
            memory_used_min: mem_avg - 10,
            memory_used_max: mem_avg + 10,
            cpu: Default::default(),
            ram: Default::default(),
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
            smart: vec![],
        };
    let aggs = vec![
        one_min(300_000, 10.0, 100),
        one_min(360_000, 20.0, 200),
        one_min(420_000, 30.0, 300),
        one_min(480_000, 40.0, 400),
        one_min(540_000, 50.0, 500),
    ];
    let out =
        homeserver::history_repo::aggregation::aggregate_aggregated_snapshots(&aggs, 300_000, 300)
            .unwrap();
    assert_eq!(out.created_at, 300_000);
    assert_eq!(out.resolution_seconds, 300);
    assert_eq!(out.cpu_load_avg, 30.0);
    assert_eq!(out.cpu_load_min, 9.0);
    assert_eq!(out.cpu_load_max, 51.0);
    assert_eq!(out.memory_used_avg, 300);
    assert_eq!(out.memory_used_min, 90);
    assert_eq!(out.memory_used_max, 510);
}

/// A snapshot whose full CPU/RAM/GPU/SMART detail is distinctive, to verify it survives aggregation.
fn rich_snapshot(ts: u64) -> FullSystemSnapshot {
    FullSystemSnapshot {
        timestamp: ts,
        cpu: CpuStats {
            model: "Rich CPU".into(),
            physical_cores: 8,
            logical_cores: 16,
            usage_percent: 42.0,
            temperature: 65.5,
            core_usages: vec![1.0, 2.0],
        },
        ram: RamStats {
            total: 32_000,
            used: 8_000,
            available: 24_000,
            usage_percent: 25.0,
            swap_total: 4_000,
            swap_used: 256,
            swap_free: 3_744,
        },
        containers: vec![],
        storage: StorageStats::default(),
        network: NetworkStats::default(),
        system: SystemStatsDynamic::default(),
        gpus: vec![GpuStats {
            index: 0,
            vendor: "amd".into(),
            name: "RX".into(),
            utilization_percent: 55.0,
            memory_used_bytes: 1024,
            memory_total_bytes: 8192,
            temperature_c: 70.0,
            power_watts: Some(120.0),
            fan_percent: Some(40.0),
        }],
        smart: vec![SmartHealth {
            device: "/dev/sda".into(),
            model: "Disk".into(),
            health_passed: true,
            temperature_c: Some(38),
            power_on_hours: Some(42),
            reallocated_sectors: Some(0),
            wear_level_percent: Some(3),
        }],
    }
}

#[test]
fn aggregate_snapshots_carries_full_detail_from_last_sample() {
    // First sample is sparse; the last (rich) sample's full structs must be the ones kept.
    let snaps = vec![snapshot(60_000, 10.0, 100), rich_snapshot(60_001)];
    let out = aggregate_snapshots(&snaps, 60_000, 60).unwrap();
    // Scalars still aggregate across the bucket...
    assert_eq!(out.cpu_load_min, 10.0);
    // ...but cpu/ram/gpu/smart come verbatim from the last sample.
    assert_eq!(out.cpu.model, "Rich CPU");
    assert!((out.cpu.temperature - 65.5).abs() < 0.001);
    assert_eq!(out.ram.swap_total, 4_000);
    assert_eq!(out.gpus.len(), 1);
    assert_eq!(out.gpus[0].vendor, "amd");
    assert_eq!(out.gpus[0].power_watts, Some(120.0));
    assert_eq!(out.smart.len(), 1);
    assert_eq!(out.smart[0].device, "/dev/sda");
    assert_eq!(out.smart[0].wear_level_percent, Some(3));
}
