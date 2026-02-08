// One-time backfill: run one aggregation pass at startup to roll existing raw/1-min data.

use crate::aggregation_worker::{AggregationWorkerConfig, run_one_tick};
use crate::history_repo::HistoryRepo;
use std::sync::Arc;
use tracing::info;

/// Runs one aggregation pass to backfill existing raw data into 1-min and 5-min buckets.
pub async fn run_backfill(
    repo: Arc<HistoryRepo>,
    config: &AggregationWorkerConfig,
) -> anyhow::Result<()> {
    run_one_tick(repo.as_ref(), config).await?;
    info!("backfill complete");
    Ok(())
}
