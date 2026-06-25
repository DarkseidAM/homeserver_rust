// Dedicated history flush task fed by snapshot channel.

use crate::history_repo::HistoryRepo;
use crate::models::{FullSystemSnapshot, SystemInfo};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};

use super::HistoryWriterConfig;

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
    let persist_gpu = config.persist_gpu;
    let persist_smart = config.persist_smart;
    tokio::spawn(async move {
        let mut buffer: Vec<FullSystemSnapshot> = Vec::new();
        let mut flush_tick = interval(flush_interval);
        flush_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                result = write_rx.recv() => {
                    match result {
                        Some(mut snapshot) => {
                            if !persist_gpu {
                                snapshot.gpus.clear();
                            }
                            if !persist_smart {
                                snapshot.smart.clear();
                            }
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
