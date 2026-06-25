// API history merge path, deserialization helpers for stored blobs, VACUUM.

use crate::history_repo::{HistoryRepo, blob};
use crate::models::{
    AggregatedSnapshot, ContainerStats, CpuStats, FullSystemSnapshot, NetworkStats, RamStats,
    StorageStats,
};
use std::collections::BTreeMap;
use tracing::instrument;

/// Deserialize container_data; on legacy/corrupt blob return empty vec and log.
pub(in crate::history_repo) fn deserialize_container_data(bytes: &[u8]) -> Vec<ContainerStats> {
    wincode::deserialize(blob::blob_payload(bytes, blob::BLOB_VERSION)).unwrap_or_else(|e| {
        tracing::debug!(error = %e, "wincode deserialize containers (legacy/corrupt), using empty");
        vec![]
    })
}

pub(in crate::history_repo) fn deserialize_storage_data(bytes: &[u8]) -> StorageStats {
    wincode::deserialize(blob::blob_payload(bytes, blob::BLOB_VERSION)).unwrap_or_else(|e| {
        tracing::debug!(error = %e, "wincode deserialize storage (legacy/corrupt), using empty");
        StorageStats {
            partitions: vec![],
            disks: vec![],
        }
    })
}

pub(in crate::history_repo) fn deserialize_network_data(bytes: &[u8]) -> NetworkStats {
    wincode::deserialize(blob::blob_payload(bytes, blob::BLOB_VERSION)).unwrap_or_else(|e| {
        tracing::debug!(error = %e, "wincode deserialize network (legacy/corrupt), using empty");
        NetworkStats { interfaces: vec![] }
    })
}

/// Deserialize the optional `cpu_data` blob. For legacy rows (NULL/empty) or corrupt data,
/// reconstruct a minimal `CpuStats` from the scalar `cpu_load` column.
pub(in crate::history_repo) fn deserialize_cpu_data(
    bytes: Option<&[u8]>,
    fallback_usage_percent: f64,
) -> CpuStats {
    match bytes {
        Some(b) if !b.is_empty() => wincode::deserialize(blob::blob_payload(b, blob::BLOB_VERSION))
            .unwrap_or_else(|e| {
                tracing::debug!(error = %e, "wincode deserialize cpu (legacy/corrupt), using scalar fallback");
                CpuStats {
                    usage_percent: fallback_usage_percent,
                    ..Default::default()
                }
            }),
        _ => CpuStats {
            usage_percent: fallback_usage_percent,
            ..Default::default()
        },
    }
}

/// Deserialize the optional `ram_data` blob. For legacy rows (NULL/empty) or corrupt data,
/// reconstruct a minimal `RamStats` from the scalar `memory_used` column.
pub(in crate::history_repo) fn deserialize_ram_data(
    bytes: Option<&[u8]>,
    fallback_used: u64,
) -> RamStats {
    match bytes {
        Some(b) if !b.is_empty() => wincode::deserialize(blob::blob_payload(b, blob::BLOB_VERSION))
            .unwrap_or_else(|e| {
                tracing::debug!(error = %e, "wincode deserialize ram (legacy/corrupt), using scalar fallback");
                RamStats {
                    used: fallback_used,
                    ..Default::default()
                }
            }),
        _ => RamStats {
            used: fallback_used,
            ..Default::default()
        },
    }
}

pub(in crate::history_repo) fn aggregated_to_snapshot(
    agg: AggregatedSnapshot,
) -> FullSystemSnapshot {
    FullSystemSnapshot {
        timestamp: agg.created_at as u64,
        cpu: agg.cpu,
        ram: agg.ram,
        containers: agg.containers,
        storage: agg.storage,
        network: agg.network,
        system: agg.system,
    }
}

pub(in crate::history_repo) fn downsample_snapshots(
    snapshots: &[FullSystemSnapshot],
    resolution_ms: i64,
) -> Vec<FullSystemSnapshot> {
    if snapshots.is_empty() || resolution_ms <= 0 {
        return snapshots.to_vec();
    }
    let mut by_bucket: BTreeMap<i64, &FullSystemSnapshot> = BTreeMap::new();
    for s in snapshots {
        let bucket = (s.timestamp as i64 / resolution_ms) * resolution_ms;
        by_bucket.insert(bucket, s);
    }
    by_bucket.into_values().cloned().collect()
}

impl HistoryRepo {
    /// History for API: merge raw (recent) + aggregated (older) by time range and resolution.
    /// raw_cutoff_ts: timestamps >= this are read from raw table; older from aggregated (60 or 300s).
    /// resolution_secs: 1, 30, 60, 300. Raw is downsampled to this if > 1.
    #[instrument(skip(self), fields(repo = "history", operation = "get_history"))]
    pub async fn get_history(
        &self,
        from_ts: i64,
        to_ts: i64,
        resolution_secs: u32,
        raw_cutoff_ts: i64,
    ) -> anyhow::Result<Vec<FullSystemSnapshot>> {
        let resolution_ms = (resolution_secs as i64) * 1000;

        let mut raw = if to_ts > raw_cutoff_ts {
            let raw_from = from_ts.max(raw_cutoff_ts);
            self.get_raw_snapshots_by_time_range(raw_from, to_ts)
                .await?
        } else {
            Vec::new()
        };

        if resolution_secs > 1 && !raw.is_empty() {
            raw = downsample_snapshots(&raw, resolution_ms);
        }

        let agg_snapshots: Vec<FullSystemSnapshot> = if from_ts < raw_cutoff_ts {
            let agg_to = to_ts.min(raw_cutoff_ts);
            let agg_resolution = if resolution_secs >= 300 { 300 } else { 60 };
            let aggs = self
                .get_aggregated_snapshots_by_time_range(from_ts, agg_to, agg_resolution)
                .await?;
            aggs.into_iter().map(aggregated_to_snapshot).collect()
        } else {
            Vec::new()
        };

        let mut out = Vec::with_capacity(agg_snapshots.len() + raw.len());
        out.extend(agg_snapshots);
        out.extend(raw);
        out.sort_by_key(|s| s.timestamp);
        Ok(out)
    }

    /// Reclaim space after deletes (run periodically after pruning).
    #[instrument(skip(self), fields(repo = "history", operation = "vacuum"))]
    pub async fn vacuum(&self) -> anyhow::Result<()> {
        sqlx::query("VACUUM").execute(&self.pool).await?;
        Ok(())
    }

    /// Cheap liveness check: verifies a connection can be acquired and queried.
    pub async fn ping(&self) -> anyhow::Result<()> {
        sqlx::query_scalar::<_, i64>("SELECT 1")
            .fetch_one(&self.pool)
            .await?;
        Ok(())
    }
}
