// Process raw Docker stats API response into ContainerStats.

use crate::models::{ContainerState, ContainerStats};
use bollard::models::ContainerStatsResponse;

/// Process a raw Docker stats response into [`ContainerStats`].
pub fn process_statistics(
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
