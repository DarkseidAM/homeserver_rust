// Background worker: roll raw 1s → 1-min, then 1-min → 5-min, then prune.
// Runs every aggregation_interval_secs when enable_aggregation is true.
// VACUUM runs on a configurable schedule (cron expression or fixed interval).

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use crate::history_repo::HistoryRepo;
use crate::history_repo::aggregation;
use tracing::{info, instrument, warn};

const MS_PER_MINUTE: i64 = 60_000;
const MS_PER_5_MINUTES: i64 = 300_000;
const RESOLUTION_1MIN: i32 = 60;
const RESOLUTION_5MIN: i32 = 300;

/// Config for the aggregation worker.
#[derive(Debug, Clone)]
pub struct AggregationWorkerConfig {
    pub aggregation_interval_secs: u64,
    pub raw_retention_hours: u32,
    pub minute_retention_hours: u32,
    pub retention_days: u32,
    /// Optional cron expression for VACUUM (e.g. "0 3 * * *" = 03:00 daily). Uses local time.
    pub vacuum_schedule: Option<String>,
    /// Run VACUUM every N seconds when vacuum_schedule is not set.
    pub vacuum_interval_secs: u64,
}

/// Spawns the aggregation worker. Returns a join handle.
pub fn spawn(
    repo: Arc<HistoryRepo>,
    config: AggregationWorkerConfig,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        run(repo, config).await;
    })
}

#[instrument(skip(repo), fields(interval_secs = config.aggregation_interval_secs))]
async fn run(repo: Arc<HistoryRepo>, config: AggregationWorkerConfig) {
    let mut agg_interval =
        tokio::time::interval(Duration::from_secs(config.aggregation_interval_secs));
    agg_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let (vacuum_tx, mut vacuum_rx) = tokio::sync::mpsc::channel::<()>(1);
    tokio::spawn(vacuum_scheduler(config.clone(), vacuum_tx));

    loop {
        tokio::select! {
            _ = agg_interval.tick() => {
                if let Err(e) = run_one_tick(&repo, &config).await {
                    warn!(error = %e, "aggregation tick failed");
                }
            }
            _ = vacuum_rx.recv() => {
                if let Err(e) = repo.vacuum().await {
                    warn!(error = %e, "vacuum failed");
                } else {
                    info!("vacuum complete");
                }
            }
        }
    }
}

/// Sends a message on `tx` at each VACUUM time (cron or fixed interval). Uses local time for cron.
async fn vacuum_scheduler(config: AggregationWorkerConfig, tx: tokio::sync::mpsc::Sender<()>) {
    if let Some(ref cron_str) = config.vacuum_schedule {
        let Ok(schedule) = cron::Schedule::from_str(cron_str) else {
            warn!(cron = %cron_str, "invalid vacuum_schedule; VACUUM will not run");
            return;
        };
        loop {
            let now = chrono::Local::now();
            let next = schedule.after(&now).next();
            if let Some(next) = next {
                let delay = (next - now).to_std().unwrap_or(Duration::from_secs(1));
                tokio::time::sleep(delay).await;
                if tx.send(()).await.is_err() {
                    break;
                }
            } else {
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        }
    } else {
        let interval = Duration::from_secs(config.vacuum_interval_secs);
        loop {
            tokio::time::sleep(interval).await;
            if tx.send(()).await.is_err() {
                break;
            }
        }
    }
}

/// Runs one aggregation pass (raw→1min, 1min→5min, prune). Used by worker loop and by backfill.
pub async fn run_one_tick(
    repo: &HistoryRepo,
    config: &AggregationWorkerConfig,
) -> anyhow::Result<()> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as i64;
    let cutoff_raw = now_ms - (config.raw_retention_hours as i64) * 3600 * 1000;

    let Some(min_ts) = repo.get_min_raw_created_at_before(cutoff_raw).await? else {
        return Ok(());
    };

    let bucket_start_floor = (min_ts / MS_PER_MINUTE) * MS_PER_MINUTE;
    let mut bucket_start = bucket_start_floor;
    let mut aggregated_count: u32 = 0;

    while bucket_start + MS_PER_MINUTE <= cutoff_raw {
        let bucket_end = bucket_start + MS_PER_MINUTE;
        let snapshots = repo
            .get_raw_snapshots_by_time_range(bucket_start, bucket_end)
            .await?;

        if let Some(agg) =
            aggregation::aggregate_snapshots(&snapshots, bucket_start, RESOLUTION_1MIN)
        {
            repo.save_aggregated_snapshot(&agg).await?;
            aggregated_count += 1;
        }
        let _ = repo.delete_raw_range(bucket_start, bucket_end).await?;
        bucket_start += MS_PER_MINUTE;
    }

    if aggregated_count > 0 {
        info!(
            aggregated_buckets = aggregated_count,
            "raw -> 1-min aggregation"
        );
    }

    // 1-min → 5-min: roll up 1-min rows older than minute_retention_hours into 5-min buckets
    let cutoff_1min = now_ms - (config.minute_retention_hours as i64) * 3600 * 1000;
    let Some(min_1min_ts) = repo
        .get_min_aggregated_created_at_before(cutoff_1min, RESOLUTION_1MIN)
        .await?
    else {
        repo.prune_old_data().await?;
        repo.prune_aggregated_old_data().await?;
        return Ok(());
    };

    let bucket_start_floor = (min_1min_ts / MS_PER_5_MINUTES) * MS_PER_5_MINUTES;
    let mut bucket_start = bucket_start_floor;
    let mut rolled_up_count: u32 = 0;

    while bucket_start + MS_PER_5_MINUTES <= cutoff_1min {
        let bucket_end = bucket_start + MS_PER_5_MINUTES;
        let one_min_rows = repo
            .get_aggregated_snapshots_by_time_range(bucket_start, bucket_end, RESOLUTION_1MIN)
            .await?;

        if let Some(agg_5min) = aggregation::aggregate_aggregated_snapshots(
            &one_min_rows,
            bucket_start,
            RESOLUTION_5MIN,
        ) {
            repo.save_aggregated_snapshot(&agg_5min).await?;
            rolled_up_count += 1;
        }
        let _ = repo
            .delete_aggregated_range(bucket_start, bucket_end, RESOLUTION_1MIN)
            .await?;
        bucket_start += MS_PER_5_MINUTES;
    }

    if rolled_up_count > 0 {
        info!(
            rolled_up_buckets = rolled_up_count,
            "1-min -> 5-min aggregation"
        );
    }

    repo.prune_old_data().await?;
    repo.prune_aggregated_old_data().await?;

    Ok(())
}
