// Downsampling: schema for aggregated table + pure aggregation logic.
// DB access (get by range, save, delete) stays in history_repo::mod.

use std::collections::HashMap;

use crate::models::{AggregatedSnapshot, ContainerStats, FullSystemSnapshot};
use sqlx::SqlitePool;

/// Creates the system_history_aggregated table and index if not present.
pub async fn init_aggregated_table(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS system_history_aggregated (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at INTEGER NOT NULL,
            resolution_seconds INTEGER NOT NULL,
            cpu_load_avg REAL NOT NULL,
            cpu_load_min REAL,
            cpu_load_max REAL,
            memory_used_avg INTEGER NOT NULL,
            memory_used_min INTEGER,
            memory_used_max INTEGER,
            container_data BLOB NOT NULL,
            storage_data BLOB NOT NULL,
            network_data BLOB NOT NULL,
            system_data BLOB NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_aggregated_created_at_resolution ON system_history_aggregated(created_at, resolution_seconds)",
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Aggregates a bucket of raw snapshots into one AggregatedSnapshot.
/// Uses bucket_start_ts as created_at; resolution_seconds is 60 or 300.
pub fn aggregate_snapshots(
    snapshots: &[FullSystemSnapshot],
    bucket_start_ts: i64,
    resolution_seconds: i32,
) -> Option<AggregatedSnapshot> {
    if snapshots.is_empty() {
        return None;
    }

    let cpu_loads: Vec<f64> = snapshots.iter().map(|s| s.cpu.usage_percent).collect();
    let memory_used: Vec<i64> = snapshots.iter().map(|s| s.ram.used as i64).collect();

    let cpu_load_avg = mean_f64(&cpu_loads);
    let cpu_load_min = cpu_loads.iter().copied().fold(f64::INFINITY, f64::min);
    let cpu_load_max = cpu_loads.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    let memory_used_avg = mean_i64(&memory_used);
    let memory_used_min = *memory_used.iter().min().unwrap_or(&0);
    let memory_used_max = *memory_used.iter().max().unwrap_or(&0);

    let containers = aggregate_containers(snapshots);
    let last = snapshots.last().unwrap();
    let storage = last.storage.clone();
    let network = last.network.clone();
    let system = last.system.clone();

    Some(AggregatedSnapshot {
        created_at: bucket_start_ts,
        resolution_seconds,
        cpu_load_avg,
        cpu_load_min,
        cpu_load_max,
        memory_used_avg,
        memory_used_min,
        memory_used_max,
        containers,
        storage,
        network,
        system,
    })
}

/// Aggregates a bucket of 1-min aggregated snapshots into one 5-min AggregatedSnapshot.
pub fn aggregate_aggregated_snapshots(
    aggs: &[AggregatedSnapshot],
    bucket_start_ts: i64,
    resolution_seconds: i32,
) -> Option<AggregatedSnapshot> {
    if aggs.is_empty() {
        return None;
    }

    let cpu_load_avg = mean_f64(&aggs.iter().map(|a| a.cpu_load_avg).collect::<Vec<_>>());
    let cpu_load_min = aggs
        .iter()
        .map(|a| a.cpu_load_min)
        .fold(f64::INFINITY, f64::min);
    let cpu_load_max = aggs
        .iter()
        .map(|a| a.cpu_load_max)
        .fold(f64::NEG_INFINITY, f64::max);

    let memory_used_avg = mean_i64(&aggs.iter().map(|a| a.memory_used_avg).collect::<Vec<_>>());
    let memory_used_min = aggs.iter().map(|a| a.memory_used_min).min().unwrap_or(0);
    let memory_used_max = aggs.iter().map(|a| a.memory_used_max).max().unwrap_or(0);

    let containers = aggregate_containers_from_aggregated(aggs);
    let last = aggs.last().unwrap();
    let storage = last.storage.clone();
    let network = last.network.clone();
    let system = last.system.clone();

    Some(AggregatedSnapshot {
        created_at: bucket_start_ts,
        resolution_seconds,
        cpu_load_avg,
        cpu_load_min,
        cpu_load_max,
        memory_used_avg,
        memory_used_min,
        memory_used_max,
        containers,
        storage,
        network,
        system,
    })
}

/// Group by container id across aggregated snapshots; for each container call aggregate_one_container.
fn aggregate_containers_from_aggregated(aggs: &[AggregatedSnapshot]) -> Vec<ContainerStats> {
    let mut by_id: HashMap<String, Vec<&ContainerStats>> = HashMap::new();
    for a in aggs {
        for c in &a.containers {
            by_id.entry(c.id.clone()).or_default().push(c);
        }
    }
    let mut out: Vec<ContainerStats> = Vec::with_capacity(by_id.len());
    for (_id, refs) in by_id {
        if refs.is_empty() {
            continue;
        }
        out.push(aggregate_one_container(&refs));
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Group by container id; for each container compute avg (gauges), sum (counters), last (state/pids).
fn aggregate_containers(snapshots: &[FullSystemSnapshot]) -> Vec<ContainerStats> {
    type Key = String;
    let mut by_id: HashMap<Key, Vec<&ContainerStats>> = HashMap::new();
    for s in snapshots {
        for c in &s.containers {
            by_id.entry(c.id.clone()).or_default().push(c);
        }
    }

    let mut out: Vec<ContainerStats> = Vec::with_capacity(by_id.len());
    for (_id, refs) in by_id {
        if refs.is_empty() {
            continue;
        }
        let c = aggregate_one_container(&refs);
        out.push(c);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn aggregate_one_container(refs: &[&ContainerStats]) -> ContainerStats {
    let first = refs[0];

    let cpu_percent_avg = mean_f64(&refs.iter().map(|c| c.cpu_percent).collect::<Vec<_>>());
    let memory_usage_avg = mean_u64(
        &refs
            .iter()
            .map(|c| c.memory_usage_bytes)
            .collect::<Vec<_>>(),
    );
    let memory_limit_avg = mean_u64(
        &refs
            .iter()
            .map(|c| c.memory_limit_bytes)
            .collect::<Vec<_>>(),
    );

    let network_rx_bytes: u64 = refs.iter().map(|c| c.network_rx_bytes).sum();
    let network_tx_bytes: u64 = refs.iter().map(|c| c.network_tx_bytes).sum();
    let network_rx_packets: u64 = refs.iter().map(|c| c.network_rx_packets).sum();
    let network_tx_packets: u64 = refs.iter().map(|c| c.network_tx_packets).sum();
    let block_read_bytes: u64 = refs.iter().map(|c| c.block_read_bytes).sum();
    let block_write_bytes: u64 = refs.iter().map(|c| c.block_write_bytes).sum();
    let cpu_throttled_periods: u64 = refs.iter().map(|c| c.cpu_throttled_periods).sum();
    let cpu_throttled_time_ns: u64 = refs.iter().map(|c| c.cpu_throttled_time_ns).sum();

    let cpu_kernel_avg = mean_f64(
        &refs
            .iter()
            .map(|c| c.cpu_kernel_percent)
            .collect::<Vec<_>>(),
    );
    let cpu_user_avg = mean_f64(&refs.iter().map(|c| c.cpu_user_percent).collect::<Vec<_>>());

    let last = refs[refs.len() - 1];
    ContainerStats {
        id: first.id.clone(),
        name: first.name.clone(),
        cpu_percent: cpu_percent_avg,
        memory_usage_bytes: memory_usage_avg,
        memory_limit_bytes: memory_limit_avg,
        state: last.state,
        network_rx_bytes,
        network_tx_bytes,
        network_rx_packets,
        network_tx_packets,
        network_rx_errors: last.network_rx_errors,
        network_tx_errors: last.network_tx_errors,
        network_rx_dropped: last.network_rx_dropped,
        network_tx_dropped: last.network_tx_dropped,
        block_read_bytes,
        block_write_bytes,
        block_read_ops: last.block_read_ops,
        block_write_ops: last.block_write_ops,
        pids: last.pids,
        pids_limit: last.pids_limit,
        cpu_throttled: last.cpu_throttled,
        cpu_throttled_periods,
        cpu_throttled_time_ns,
        cpu_kernel_percent: cpu_kernel_avg,
        cpu_user_percent: cpu_user_avg,
        online_cpus: last.online_cpus,
        memory_max_usage_bytes: last.memory_max_usage_bytes,
    }
}

fn mean_f64(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.iter().sum::<f64>() / (v.len() as f64)
}

fn mean_i64(v: &[i64]) -> i64 {
    if v.is_empty() {
        return 0;
    }
    v.iter().sum::<i64>() / (v.len() as i64)
}

fn mean_u64(v: &[u64]) -> u64 {
    if v.is_empty() {
        return 0;
    }
    v.iter().sum::<u64>() / (v.len() as u64)
}
