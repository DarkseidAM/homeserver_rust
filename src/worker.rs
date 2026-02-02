// Background stats worker (same logic as Kotlin StatsWorker)

use crate::docker_repo::DockerRepo;
use crate::history_repo::HistoryRepo;
use crate::models::FullSystemSnapshot;
use crate::sysinfo_repo::SysinfoRepo;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration, Instant};

const PRUNE_INTERVAL_TICKS: u64 = 3600;
/// Rate limit for "no receivers" warning (avoid logging every second when no one is on /ws/system)
const NO_RECEIVERS_WARN_INTERVAL: Duration = Duration::from_secs(60);

/// Repos, channels, and shutdown for the worker.
pub struct WorkerDeps {
    pub sysinfo_repo: Arc<SysinfoRepo>,
    pub docker_repo: Arc<DockerRepo>,
    pub history_repo: Arc<HistoryRepo>,
    pub tx: broadcast::Sender<FullSystemSnapshot>,
    pub ws_system_connections: Arc<AtomicUsize>,
    pub shutdown_rx: tokio::sync::oneshot::Receiver<()>,
}

/// Worker timing and logging config.
pub struct WorkerConfig {
    pub flush_rate: u64,
    pub sample_interval_ms: u64,
    pub stats_log_interval_secs: u64,
}

pub fn spawn(deps: WorkerDeps, config: WorkerConfig) -> tokio::task::JoinHandle<()> {
    let WorkerDeps {
        sysinfo_repo,
        docker_repo,
        history_repo,
        tx,
        ws_system_connections,
        mut shutdown_rx,
    } = deps;
    let WorkerConfig {
        flush_rate,
        sample_interval_ms,
        stats_log_interval_secs,
    } = config;

    tokio::spawn(async move {
        let mut tick = interval(Duration::from_millis(sample_interval_ms));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut snapshot_buffer: Vec<FullSystemSnapshot> = Vec::new();
        let mut flush_ticks: u64 = 0;
        let mut prune_ticks: u64 = 0;
        let mut stats_log_ticks: u64 = 0;
        let mut snapshots_saved_total: u64 = 0;
        let mut snapshots_pruned_total: u64 = 0;
        let mut last_no_receivers_warn: Option<Instant> = None;

        loop {
            tokio::select! {
                _ = tick.tick() => {}
                _ = &mut shutdown_rx => {
                    if !snapshot_buffer.is_empty()
                        && let Err(e) = history_repo.save_snapshots(&snapshot_buffer).await
                    {
                        tracing::warn!("Failed to save snapshots on shutdown: {}", e);
                    }
                    break;
                }
            }

            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or_else(|e| {
                    tracing::warn!("system time error: {}", e);
                    0
                });

            let cpu = match sysinfo_repo.get_cpu_stats().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("CPU stats failed: {}", e);
                    continue;
                }
            };
            let ram = match sysinfo_repo.get_ram_stats().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("RAM stats failed: {}", e);
                    continue;
                }
            };
            let containers = docker_repo.list_running_and_refresh_stats().await;
            let storage = match sysinfo_repo.get_storage_stats().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("storage stats failed: {}", e);
                    continue;
                }
            };
            let network = match sysinfo_repo.get_network_stats().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("network stats failed: {}", e);
                    continue;
                }
            };
            let system = match sysinfo_repo.get_system_stats().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("system stats failed: {}", e);
                    continue;
                }
            };

            let snapshot = FullSystemSnapshot {
                timestamp,
                cpu,
                ram,
                containers,
                storage,
                network,
                system,
            };

            if tx.send(snapshot.clone()).is_err() {
                let should_warn = last_no_receivers_warn
                    .is_none_or(|t| t.elapsed() >= NO_RECEIVERS_WARN_INTERVAL);
                if should_warn {
                    tracing::warn!("No active WebSocket clients; broadcast channel has no receivers");
                    last_no_receivers_warn = Some(Instant::now());
                }
            }
            snapshot_buffer.push(snapshot);

            flush_ticks += 1;
            if flush_ticks >= flush_rate && !snapshot_buffer.is_empty() {
                let n = snapshot_buffer.len();
                if let Err(e) = history_repo.save_snapshots(&snapshot_buffer).await {
                    tracing::warn!("Failed to save snapshots: {}", e);
                } else {
                    snapshots_saved_total += n as u64;
                }
                snapshot_buffer.clear();
                flush_ticks = 0;
            }

            prune_ticks += 1;
            if prune_ticks >= PRUNE_INTERVAL_TICKS {
                if let Err(e) = history_repo.prune_old_data().await {
                    tracing::warn!("Failed to prune old data: {}", e);
                } else {
                    snapshots_pruned_total += 1;
                }
                prune_ticks = 0;
            }

            stats_log_ticks += 1;
            if stats_log_ticks >= stats_log_interval_secs {
                tracing::info!(
                    ws_system_clients = ws_system_connections.load(std::sync::atomic::Ordering::Relaxed),
                    snapshots_saved_total = snapshots_saved_total,
                    snapshots_pruned_total = snapshots_pruned_total,
                    "app stats"
                );
                stats_log_ticks = 0;
            }
        }
    })
}
