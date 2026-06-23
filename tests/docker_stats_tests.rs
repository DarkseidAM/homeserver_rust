// Integration tests for Docker stats parsing (`process_statistics`).
// Keep production `src/` free of #[cfg(test)] per project rules.

use bollard::models::{
    ContainerBlkioStatEntry, ContainerBlkioStats, ContainerCpuStats, ContainerCpuUsage,
    ContainerMemoryStats, ContainerNetworkStats, ContainerPidsStats, ContainerStatsResponse,
    ContainerThrottlingData,
};
use homeserver::docker_repo::process_statistics;
use std::collections::HashMap;

fn minimal_cpu_stats(total_usage: u64, system_cpu_usage: u64) -> ContainerCpuStats {
    ContainerCpuStats {
        cpu_usage: Some(ContainerCpuUsage {
            total_usage: Some(total_usage),
            ..Default::default()
        }),
        system_cpu_usage: Some(system_cpu_usage),
        online_cpus: Some(2),
        throttling_data: None,
    }
}

#[test]
fn process_statistics_returns_none_when_cpu_stats_missing() {
    let s = ContainerStatsResponse {
        cpu_stats: None,
        precpu_stats: Some(minimal_cpu_stats(0, 0)),
        ..Default::default()
    };
    assert!(process_statistics(&s, "id", "name").is_none());
}

#[test]
fn process_statistics_returns_none_when_precpu_stats_missing() {
    let s = ContainerStatsResponse {
        cpu_stats: Some(minimal_cpu_stats(100, 1000)),
        precpu_stats: None,
        ..Default::default()
    };
    assert!(process_statistics(&s, "id", "name").is_none());
}

#[test]
fn process_statistics_computes_cpu_and_memory() {
    let s = ContainerStatsResponse {
        cpu_stats: Some(minimal_cpu_stats(100_000_000, 1_000_000_000)),
        precpu_stats: Some(minimal_cpu_stats(50_000_000, 500_000_000)),
        memory_stats: Some(ContainerMemoryStats {
            usage: Some(256 * 1024 * 1024),
            limit: Some(512 * 1024 * 1024),
            max_usage: Some(300 * 1024 * 1024),
            ..Default::default()
        }),
        networks: Some({
            let mut m = HashMap::new();
            m.insert(
                "eth0".to_string(),
                ContainerNetworkStats {
                    rx_bytes: Some(1000),
                    tx_bytes: Some(2000),
                    ..Default::default()
                },
            );
            m
        }),
        pids_stats: Some(ContainerPidsStats {
            current: Some(5),
            ..Default::default()
        }),
        blkio_stats: Some(ContainerBlkioStats {
            io_service_bytes_recursive: Some(vec![
                ContainerBlkioStatEntry {
                    op: Some("read".to_string()),
                    value: Some(100),
                    ..Default::default()
                },
                ContainerBlkioStatEntry {
                    op: Some("write".to_string()),
                    value: Some(200),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let out = process_statistics(&s, "abc123", "mycontainer").unwrap();
    assert_eq!(out.id, "abc123");
    assert_eq!(out.name, "mycontainer");
    assert!((out.cpu_percent - 20.0).abs() < 0.01);
    assert_eq!(out.memory_usage_bytes, 256 * 1024 * 1024);
    assert_eq!(out.memory_limit_bytes, 512 * 1024 * 1024);
    assert_eq!(out.memory_max_usage_bytes, 300 * 1024 * 1024);
    assert_eq!(out.network_rx_bytes, 1000);
    assert_eq!(out.network_tx_bytes, 2000);
    assert_eq!(out.pids, 5);
    assert_eq!(out.block_read_bytes, 100);
    assert_eq!(out.block_write_bytes, 200);
    assert!(!out.cpu_throttled);
}

#[test]
fn process_statistics_detects_throttling() {
    let s = ContainerStatsResponse {
        cpu_stats: Some(ContainerCpuStats {
            cpu_usage: Some(ContainerCpuUsage {
                total_usage: Some(100),
                ..Default::default()
            }),
            system_cpu_usage: Some(1000),
            online_cpus: Some(1),
            throttling_data: Some(ContainerThrottlingData {
                throttled_periods: Some(1),
                ..Default::default()
            }),
        }),
        precpu_stats: Some(minimal_cpu_stats(50, 500)),
        ..Default::default()
    };
    let out = process_statistics(&s, "x", "y").unwrap();
    assert!(out.cpu_throttled);
}

#[test]
fn process_statistics_zero_system_delta_returns_zero_cpu_percent() {
    let s = ContainerStatsResponse {
        cpu_stats: Some(minimal_cpu_stats(100, 500)),
        precpu_stats: Some(minimal_cpu_stats(50, 500)),
        ..Default::default()
    };
    let out = process_statistics(&s, "id", "n").unwrap();
    assert_eq!(out.cpu_percent, 0.0);
}
