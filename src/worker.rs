// Background stats worker (same logic as Kotlin StatsWorker).
// Collection runs in the worker; persistence runs in a dedicated history writer task (channel).

use crate::docker_repo::DockerRepo;
use crate::history_repo::HistoryRepo;
use crate::models::{FullSystemSnapshot, SystemInfo};
use crate::sysinfo_repo::SysinfoRepo;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize};
use tokio::sync::{broadcast, mpsc};
use tokio::time::{Duration, Instant, interval};

/// Rate limit for "no receivers" warning (avoid logging every second when no one is on /ws/system)
const NO_RECEIVERS_WARN_INTERVAL: Duration = Duration::from_secs(60);

/// Channel capacity for snapshot writer (backpressure if writer falls behind).
pub fn writer_channel_capacity(flush_rate: u64) -> usize {
    (flush_rate as usize * 2).max(32)
}

/// Repos, channels, and shutdown for the worker.
pub struct WorkerDeps {
    pub sysinfo_repo: Arc<SysinfoRepo>,
    pub system_info: Arc<SystemInfo>,
    pub docker_repo: Arc<DockerRepo>,
    pub history_repo: Arc<HistoryRepo>,
    pub tx: broadcast::Sender<FullSystemSnapshot>,
    pub write_tx: mpsc::Sender<FullSystemSnapshot>,
    pub ws_system_connections: Arc<AtomicUsize>,
    pub snapshots_saved_total: Arc<AtomicU64>,
    pub shutdown_rx: tokio::sync::oneshot::Receiver<()>,
}

/// Worker timing and logging config.
/// Stats logging and pruning use real-time intervals, independent of sample_interval_ms.
pub struct WorkerConfig {
    pub sample_interval_ms: u64,
    /// How often to log app stats (real seconds).
    pub stats_log_interval_secs: u64,
    /// How often to prune old data (real seconds).
    pub prune_interval_secs: u64,
}

/// Writer config: batching for the dedicated history writer task.
pub struct HistoryWriterConfig {
    pub flush_rate: u64,
    pub flush_interval_secs: u64,
}

/// Spawns the background task that receives snapshots from the worker and flushes to the DB.
/// Flushes when buffer len >= flush_rate, or every flush_interval_secs, or when channel closes.
/// When the worker drops its sender, this task flushes remaining and exits.
pub fn spawn_history_writer(
    mut write_rx: mpsc::Receiver<FullSystemSnapshot>,
    history_repo: Arc<HistoryRepo>,
    system_info: Arc<SystemInfo>,
    config: HistoryWriterConfig,
    snapshots_saved_total: Arc<AtomicU64>,
) -> tokio::task::JoinHandle<()> {
    let flush_interval = Duration::from_secs(config.flush_interval_secs);
    tokio::spawn(async move {
        let mut buffer: Vec<FullSystemSnapshot> = Vec::new();
        let mut flush_tick = interval(flush_interval);
        flush_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                result = write_rx.recv() => {
                    match result {
                        Some(snapshot) => {
                            buffer.push(snapshot);
                            if buffer.len() >= config.flush_rate as usize
                                && let Err(e) = flush_buffer(&history_repo, &system_info, &mut buffer, &snapshots_saved_total).await
                            {
                                tracing::warn!(error = %e, "history writer: save_snapshots failed");
                            }
                        }
                        None => break,
                    }
                }
                _ = flush_tick.tick() => {
                    if let Err(e) = flush_buffer(&history_repo, &system_info, &mut buffer, &snapshots_saved_total).await {
                        tracing::warn!(error = %e, "history writer: save_snapshots failed");
                    }
                }
            }
        }
        if let Err(e) = flush_buffer(
            &history_repo,
            &system_info,
            &mut buffer,
            &snapshots_saved_total,
        )
        .await
        {
            tracing::warn!(error = %e, "history writer: final flush failed");
        }
        tracing::debug!("History writer shutting down");
    })
}

async fn flush_buffer(
    history_repo: &HistoryRepo,
    system_info: &SystemInfo,
    buffer: &mut Vec<FullSystemSnapshot>,
    snapshots_saved_total: &AtomicU64,
) -> anyhow::Result<()> {
    if buffer.is_empty() {
        return Ok(());
    }
    let n = buffer.len();
    history_repo.save_snapshots(buffer, system_info).await?;
    snapshots_saved_total.fetch_add(n as u64, std::sync::atomic::Ordering::Relaxed);
    buffer.clear();
    tracing::debug!(
        operation = "save_snapshots",
        snapshots_count = n,
        "Snapshots saved"
    );
    Ok(())
}

pub fn spawn(deps: WorkerDeps, config: WorkerConfig) -> tokio::task::JoinHandle<()> {
    let WorkerDeps {
        sysinfo_repo,
        system_info: _,
        docker_repo,
        history_repo,
        tx,
        write_tx,
        ws_system_connections,
        snapshots_saved_total,
        mut shutdown_rx,
    } = deps;
    let WorkerConfig {
        sample_interval_ms,
        stats_log_interval_secs,
        prune_interval_secs,
    } = config;

    let stats_log_interval = Duration::from_secs(stats_log_interval_secs);
    let prune_interval = Duration::from_secs(prune_interval_secs);

    tokio::spawn(async move {
        let mut tick = interval(Duration::from_millis(sample_interval_ms));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut stats_log_tick = interval(stats_log_interval);
        stats_log_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut prune_tick = interval(prune_interval);
        prune_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let mut snapshots_pruned_total: u64 = 0;
        let mut last_no_receivers_warn: Option<Instant> = None;

        let worker_span = tracing::span!(tracing::Level::DEBUG, "worker", sample_interval_ms);
        let _guard = worker_span.enter();

        loop {
            tokio::select! {
                _ = tick.tick() => {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        error = %e,
                        operation = "get_timestamp",
                        "system time error"
                    );
                    0
                });

            let cpu = match sysinfo_repo.get_cpu_stats().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        operation = "get_cpu_stats",
                        "CPU stats failed"
                    );
                    continue;
                }
            };
            let ram = match sysinfo_repo.get_ram_stats().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        operation = "get_ram_stats",
                        "RAM stats failed"
                    );
                    continue;
                }
            };
            let containers = docker_repo.list_running_and_refresh_stats().await;
            let storage = match sysinfo_repo.get_storage_stats().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        operation = "get_storage_stats",
                        "storage stats failed"
                    );
                    continue;
                }
            };
            let network = match sysinfo_repo.get_network_stats().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        operation = "get_network_stats",
                        "network stats failed"
                    );
                    continue;
                }
            };
            let system = match sysinfo_repo.get_system_stats().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        operation = "get_system_stats",
                        "system stats failed"
                    );
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
                    tracing::debug!(
                        operation = "broadcast_snapshot",
                        "No active WebSocket clients; broadcast channel has no receivers"
                    );
                    last_no_receivers_warn = Some(Instant::now());
                }
            }
            if write_tx.send(snapshot).await.is_err() {
                tracing::debug!("History writer channel closed");
            }
                }
                _ = &mut shutdown_rx => {
                    tracing::debug!("Worker shutting down");
                    break;
                }
                _ = stats_log_tick.tick() => {
                    tracing::info!(
                        ws_system_clients =
                            ws_system_connections.load(std::sync::atomic::Ordering::Relaxed),
                        snapshots_saved_total = snapshots_saved_total.load(std::sync::atomic::Ordering::Relaxed),
                        snapshots_pruned_total = snapshots_pruned_total,
                        "app stats"
                    );
                }
                _ = prune_tick.tick() => {
                    if let Err(e) = history_repo.prune_old_data().await {
                        tracing::warn!(
                            error = %e,
                            operation = "prune_old_data",
                            "Failed to prune old data"
                        );
                    } else {
                        tracing::debug!(operation = "prune_old_data", "Old data pruned successfully");
                        snapshots_pruned_total += 1;
                    }
                }
            }
        }
    })
}
