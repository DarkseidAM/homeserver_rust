// Process raw Docker stats API response into ContainerStats.

use crate::models::{ContainerState, ContainerStats};
use bollard::secret::ContainerStatsResponse;

/// Process a raw Docker stats response into our ContainerStats. Exposed for unit tests.
pub(crate) fn process_statistics(
    s: &ContainerStatsResponse,
    id: &str,
    name: &str,
) -> Option<ContainerStats> {
    let cpu_stats = s.cpu_stats.as_ref()?;
    let precpu_stats = s.precpu_stats.as_ref()?;

    let cpu_usage = cpu_stats.cpu_usage.as_ref()?;
    let precpu_usage = precpu_stats.cpu_usage.as_ref()?;

    let cpu_delta =
        cpu_usage.total_usage.unwrap_or(0) as i64 - precpu_usage.total_usage.unwrap_or(0) as i64;
    let kernel_delta = cpu_usage.usage_in_kernelmode.unwrap_or(0) as i64
        - precpu_usage.usage_in_kernelmode.unwrap_or(0) as i64;
    let user_delta = cpu_usage.usage_in_usermode.unwrap_or(0) as i64
        - precpu_usage.usage_in_usermode.unwrap_or(0) as i64;
    let system_delta_check = cpu_stats.system_cpu_usage.unwrap_or(0) as i64
        - precpu_stats.system_cpu_usage.unwrap_or(0) as i64;
    let online = cpu_stats.online_cpus.unwrap_or(1) as f64;
    let online_cpus = cpu_stats.online_cpus.unwrap_or(1);
    let cpu_percent = if system_delta_check > 0 && online > 0.0 {
        (cpu_delta as f64 / system_delta_check as f64) * online * 100.0
    } else {
        0.0
    };
    let cpu_kernel_percent = if system_delta_check > 0 && online > 0.0 {
        (kernel_delta as f64 / system_delta_check as f64) * online * 100.0
    } else {
        0.0
    };
    let cpu_user_percent = if system_delta_check > 0 && online > 0.0 {
        (user_delta as f64 / system_delta_check as f64) * online * 100.0
    } else {
        0.0
    };

    let mem_usage = s.memory_stats.as_ref().and_then(|m| m.usage).unwrap_or(0);
    let mem_limit = s.memory_stats.as_ref().and_then(|m| m.limit).unwrap_or(0);
    let mem_max = s
        .memory_stats
        .as_ref()
        .and_then(|m| m.max_usage)
        .unwrap_or(0);

    let (
        network_rx,
        network_tx,
        network_rx_packets,
        network_tx_packets,
        network_rx_errors,
        network_tx_errors,
        network_rx_dropped,
        network_tx_dropped,
    ) = s
        .networks
        .as_ref()
        .map_or((0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64), |n| {
            let mut rx_bytes = 0u64;
            let mut tx_bytes = 0u64;
            let mut rx_packets = 0u64;
            let mut tx_packets = 0u64;
            let mut rx_errors = 0u64;
            let mut tx_errors = 0u64;
            let mut rx_dropped = 0u64;
            let mut tx_dropped = 0u64;
            for v in n.values() {
                rx_bytes += v.rx_bytes.unwrap_or(0);
                tx_bytes += v.tx_bytes.unwrap_or(0);
                rx_packets += v.rx_packets.unwrap_or(0);
                tx_packets += v.tx_packets.unwrap_or(0);
                rx_errors += v.rx_errors.unwrap_or(0);
                tx_errors += v.tx_errors.unwrap_or(0);
                rx_dropped += v.rx_dropped.unwrap_or(0);
                tx_dropped += v.tx_dropped.unwrap_or(0);
            }
            (
                rx_bytes, tx_bytes, rx_packets, tx_packets, rx_errors, tx_errors, rx_dropped,
                tx_dropped,
            )
        });

    let (block_read_bytes, block_write_bytes) = s
        .blkio_stats
        .as_ref()
        .and_then(|b| b.io_service_bytes_recursive.as_ref())
        .map_or((0u64, 0u64), |b| {
            let mut read = 0u64;
            let mut write = 0u64;
            for e in b {
                if e.op
                    .as_ref()
                    .is_some_and(|op| op.eq_ignore_ascii_case("read"))
                {
                    read += e.value.unwrap_or(0);
                } else if e
                    .op
                    .as_ref()
                    .is_some_and(|op| op.eq_ignore_ascii_case("write"))
                {
                    write += e.value.unwrap_or(0);
                }
            }
            (read, write)
        });

    let (block_read_ops, block_write_ops) = s
        .blkio_stats
        .as_ref()
        .and_then(|b| b.io_serviced_recursive.as_ref())
        .map_or((0u64, 0u64), |b| {
            let mut read = 0u64;
            let mut write = 0u64;
            for e in b {
                if e.op
                    .as_ref()
                    .is_some_and(|op| op.eq_ignore_ascii_case("read"))
                {
                    read += e.value.unwrap_or(0);
                } else if e
                    .op
                    .as_ref()
                    .is_some_and(|op| op.eq_ignore_ascii_case("write"))
                {
                    write += e.value.unwrap_or(0);
                }
            }
            (read, write)
        });

    let pids = s.pids_stats.as_ref().and_then(|p| p.current).unwrap_or(0);
    let pids_limit = s.pids_stats.as_ref().and_then(|p| p.limit).unwrap_or(0);

    let throttling = cpu_stats.throttling_data.as_ref();
    let throttled = throttling.is_some_and(|t| t.throttled_periods.unwrap_or(0) > 0);
    let throttled_periods = throttling.and_then(|t| t.throttled_periods).unwrap_or(0);
    let throttled_time_ns = throttling.and_then(|t| t.throttled_time).unwrap_or(0);

    Some(ContainerStats {
        id: id.to_string(),
        name: name.to_string(),
        cpu_percent,
        cpu_kernel_percent,
        cpu_user_percent,
        online_cpus,
        memory_usage_bytes: mem_usage,
        memory_limit_bytes: mem_limit,
        state: ContainerState::Running,
        network_rx_bytes: network_rx,
        network_tx_bytes: network_tx,
        network_rx_packets,
        network_tx_packets,
        network_rx_errors,
        network_tx_errors,
        network_rx_dropped,
        network_tx_dropped,
        block_read_bytes,
        block_write_bytes,
        block_read_ops,
        block_write_ops,
        pids,
        pids_limit,
        cpu_throttled: throttled,
        cpu_throttled_periods: throttled_periods,
        cpu_throttled_time_ns: throttled_time_ns,
        memory_max_usage_bytes: mem_max,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bollard::secret::{
        ContainerBlkioStatEntry, ContainerBlkioStats, ContainerCpuStats, ContainerCpuUsage,
        ContainerMemoryStats, ContainerNetworkStats, ContainerPidsStats, ContainerStatsResponse,
        ContainerThrottlingData,
    };
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
}
