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
        },
        ram: RamStats {
            total: 0,
            used: memory_used,
            available: 0,
            usage_percent: 0.0,
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
            cpu_voltage: 0.0,
            fan_speeds: vec![],
        },
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
                cpu_voltage: 0.0,
                fan_speeds: vec![],
            },
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
