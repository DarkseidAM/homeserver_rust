// Background stats worker (same logic as Kotlin StatsWorker).
// Collection runs in the worker; persistence runs in a dedicated history writer task (channel).

mod history_writer;

use crate::docker_repo::DockerRepo;
use crate::gpu_repo::GpuRepo;
use crate::history_repo::HistoryRepo;
use crate::models::{FullSystemSnapshot, SystemInfo};
use crate::smart_repo::SmartRepo;
use crate::sysinfo_repo::SysinfoRepo;
pub use history_writer::spawn_history_writer;
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
    pub gpu_repo: Arc<GpuRepo>,
    pub smart_repo: Arc<SmartRepo>,
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
    /// Collect GPU metrics each tick.
    pub collect_gpu: bool,
    /// Collect SMART disk health (slow background poll).
    pub collect_smart: bool,
    /// How often to refresh SMART data (real seconds).
    pub smart_poll_interval_secs: u64,
}

/// Writer config: batching for the dedicated history writer task.
pub struct HistoryWriterConfig {
    pub flush_rate: u64,
    pub flush_interval_secs: u64,
    /// When false, GPU data is dropped before persisting (live WS still includes it).
    pub persist_gpu: bool,
    /// When false, SMART data is dropped before persisting (live WS still includes it).
    pub persist_smart: bool,
}

pub fn spawn(deps: WorkerDeps, config: WorkerConfig) -> tokio::task::JoinHandle<()> {
    let WorkerDeps {
        sysinfo_repo,
        system_info: _,
        docker_repo,
        gpu_repo,
        smart_repo,
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
        collect_gpu,
        collect_smart,
        smart_poll_interval_secs,
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
        let mut smart_tick = interval(Duration::from_secs(smart_poll_interval_secs.max(1)));
        smart_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

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

            // Degrade gracefully: a single failing collector falls back to defaults for that
            // metric rather than dropping the whole tick (which would lose the healthy metrics too).
            let cpu = sysinfo_repo.get_cpu_stats().await.unwrap_or_else(|e| {
                tracing::warn!(error = %e, operation = "get_cpu_stats", "CPU stats failed; using defaults");
                Default::default()
            });
            let ram = sysinfo_repo.get_ram_stats().await.unwrap_or_else(|e| {
                tracing::warn!(error = %e, operation = "get_ram_stats", "RAM stats failed; using defaults");
                Default::default()
            });
            let containers = docker_repo.list_running_and_refresh_stats().await;
            let storage = sysinfo_repo.get_storage_stats().await.unwrap_or_else(|e| {
                tracing::warn!(error = %e, operation = "get_storage_stats", "storage stats failed; using defaults");
                Default::default()
            });
            let network = sysinfo_repo.get_network_stats().await.unwrap_or_else(|e| {
                tracing::warn!(error = %e, operation = "get_network_stats", "network stats failed; using defaults");
                Default::default()
            });
            let system = sysinfo_repo.get_system_stats().await.unwrap_or_else(|e| {
                tracing::warn!(error = %e, operation = "get_system_stats", "system stats failed; using defaults");
                Default::default()
            });
            // GPU collection is cheap (small sysfs reads / NVML queries) so it runs inline.
            let gpus = if collect_gpu {
                gpu_repo.collect()
            } else {
                Vec::new()
            };
            // SMART is refreshed on its own slow cadence (smart_tick); read the cached value here.
            let smart = smart_repo.current();

            let snapshot = FullSystemSnapshot {
                timestamp,
                cpu,
                ram,
                containers,
                storage,
                network,
                system,
                gpus,
                smart,
            };

            // Only clone for the broadcast when someone is actually listening.
            if tx.receiver_count() > 0 {
                let _ = tx.send(snapshot.clone());
            } else {
                let should_warn = last_no_receivers_warn
                    .is_none_or(|t| t.elapsed() >= NO_RECEIVERS_WARN_INTERVAL);
                if should_warn {
                    tracing::debug!(
                        operation = "broadcast_snapshot",
                        "No active WebSocket clients; skipping broadcast"
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
                _ = smart_tick.tick() => {
                    // smartctl is slow/blocking; refresh in a detached task so the loop stays responsive.
                    if collect_smart {
                        let repo = smart_repo.clone();
                        tokio::spawn(async move { repo.refresh().await });
                    }
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
