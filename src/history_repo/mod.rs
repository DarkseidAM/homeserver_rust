// SQLite history. system_info table stores static SystemInfo once; merge when loading.

pub mod aggregation;
mod blob;

use crate::models::{
    AggregatedSnapshot, ContainerStats, CpuStats, FullSystemSnapshot, NetworkStats, RamStats,
    StorageStats, SystemInfo, SystemStats, SystemStatsDynamic,
};
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;
use tracing::instrument;

pub struct HistoryRepo {
    pool: SqlitePool,
    retention_ms: i64,
}

impl HistoryRepo {
    pub async fn connect(path: &str, retention_days: u32) -> anyhow::Result<Self> {
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}", path))?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_secs(5))
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);
        let pool = SqlitePoolOptions::new().connect_with(opts).await?;
        let retention_ms = (retention_days as i64) * 24 * 60 * 60 * 1000;
        Ok(Self { pool, retention_ms })
    }

    pub async fn init(&self) -> anyhow::Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schema_version (key TEXT PRIMARY KEY, value INTEGER NOT NULL)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS system_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at INTEGER NOT NULL,
                cpu_load REAL NOT NULL,
                memory_used INTEGER NOT NULL,
                container_data BLOB NOT NULL,
                storage_data BLOB NOT NULL,
                network_data BLOB NOT NULL,
                system_data BLOB NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_history_created_at ON system_history(created_at)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS system_info (id INTEGER PRIMARY KEY CHECK (id = 1), data BLOB NOT NULL)",
        )
        .execute(&self.pool)
        .await?;

        aggregation::init_aggregated_table(&self.pool).await?;

        Ok(())
    }

    #[instrument(skip(self, snapshots, system_info), fields(repo = "history", operation = "save_snapshots", snapshots_count = snapshots.len()))]
    pub async fn save_snapshots(
        &self,
        snapshots: &[FullSystemSnapshot],
        system_info: &SystemInfo,
    ) -> anyhow::Result<()> {
        if snapshots.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;

        let info_blob = wincode::serialize(system_info)
            .map_err(|e| anyhow::anyhow!("wincode system_info: {}", e))?;
        sqlx::query("INSERT OR REPLACE INTO system_info (id, data) VALUES (1, $1)")
            .bind(&info_blob)
            .execute(&mut *tx)
            .await?;

        for s in snapshots {
            let container_data = blob::with_version_prefix(
                blob::BLOB_VERSION,
                wincode::serialize(&s.containers).map_err(|e| anyhow::anyhow!("wincode: {}", e))?,
            );
            let storage_data = blob::with_version_prefix(
                blob::BLOB_VERSION,
                wincode::serialize(&s.storage).map_err(|e| anyhow::anyhow!("wincode: {}", e))?,
            );
            let network_data = blob::with_version_prefix(
                blob::BLOB_VERSION,
                wincode::serialize(&s.network).map_err(|e| anyhow::anyhow!("wincode: {}", e))?,
            );
            let system_data = blob::with_version_prefix(
                blob::BLOB_VERSION_SYSTEM_DYNAMIC,
                wincode::serialize(&s.system).map_err(|e| anyhow::anyhow!("wincode: {}", e))?,
            );
            sqlx::query(
                "INSERT INTO system_history (created_at, cpu_load, memory_used, container_data, storage_data, network_data, system_data) VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(s.timestamp as i64)
            .bind(s.cpu.usage_percent)
            .bind(s.ram.used as i64)
            .bind(&container_data)
            .bind(&storage_data)
            .bind(&network_data)
            .bind(&system_data)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    #[instrument(skip(self), fields(repo = "history", operation = "prune_old_data"))]
    pub async fn prune_old_data(&self) -> anyhow::Result<()> {
        let cutoff = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as i64)
            - self.retention_ms;
        sqlx::query("DELETE FROM system_history WHERE created_at < $1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_stored_system_info(&self) -> anyhow::Result<Option<SystemInfo>> {
        let row = sqlx::query("SELECT data FROM system_info WHERE id = 1")
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let data: Vec<u8> = row.try_get("data")?;
        let info = wincode::deserialize(&data)
            .map_err(|e| anyhow::anyhow!("wincode deserialize system_info: {}", e))?;
        Ok(Some(info))
    }

    pub async fn get_recent_snapshots(
        &self,
        limit: u32,
    ) -> anyhow::Result<(Option<SystemInfo>, Vec<FullSystemSnapshot>)> {
        let stored_info = self.get_stored_system_info().await?;

        let rows = sqlx::query(
            "SELECT created_at, cpu_load, memory_used, container_data, storage_data, network_data, system_data
             FROM system_history ORDER BY id DESC LIMIT $1",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(Self::parse_snapshot_row(&row)?);
        }
        out.reverse();
        Ok((stored_info, out))
    }

    /// Raw snapshots in [from_ts, to_ts) for aggregation. Order: ascending by created_at.
    #[instrument(
        skip(self),
        fields(repo = "history", operation = "get_raw_snapshots_by_time_range")
    )]
    pub async fn get_raw_snapshots_by_time_range(
        &self,
        from_ts: i64,
        to_ts: i64,
    ) -> anyhow::Result<Vec<FullSystemSnapshot>> {
        let rows = sqlx::query(
            "SELECT created_at, cpu_load, memory_used, container_data, storage_data, network_data, system_data
             FROM system_history WHERE created_at >= $1 AND created_at < $2 ORDER BY created_at ASC",
        )
        .bind(from_ts)
        .bind(to_ts)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(Self::parse_snapshot_row(&row)?);
        }
        Ok(out)
    }

    #[instrument(
        skip(self, agg),
        fields(repo = "history", operation = "save_aggregated_snapshot")
    )]
    pub async fn save_aggregated_snapshot(&self, agg: &AggregatedSnapshot) -> anyhow::Result<()> {
        let container_data = blob::with_version_prefix(
            blob::BLOB_VERSION,
            wincode::serialize(&agg.containers).map_err(|e| anyhow::anyhow!("wincode: {}", e))?,
        );
        let storage_data = blob::with_version_prefix(
            blob::BLOB_VERSION,
            wincode::serialize(&agg.storage).map_err(|e| anyhow::anyhow!("wincode: {}", e))?,
        );
        let network_data = blob::with_version_prefix(
            blob::BLOB_VERSION,
            wincode::serialize(&agg.network).map_err(|e| anyhow::anyhow!("wincode: {}", e))?,
        );
        let system_data = blob::with_version_prefix(
            blob::BLOB_VERSION_SYSTEM_DYNAMIC,
            wincode::serialize(&agg.system).map_err(|e| anyhow::anyhow!("wincode: {}", e))?,
        );

        sqlx::query(
            r#"
            INSERT INTO system_history_aggregated
            (created_at, resolution_seconds, cpu_load_avg, cpu_load_min, cpu_load_max,
             memory_used_avg, memory_used_min, memory_used_max,
             container_data, storage_data, network_data, system_data)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(agg.created_at)
        .bind(agg.resolution_seconds)
        .bind(agg.cpu_load_avg)
        .bind(agg.cpu_load_min)
        .bind(agg.cpu_load_max)
        .bind(agg.memory_used_avg)
        .bind(agg.memory_used_min)
        .bind(agg.memory_used_max)
        .bind(&container_data)
        .bind(&storage_data)
        .bind(&network_data)
        .bind(&system_data)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Minimum created_at in system_history with created_at < cutoff_ts (for aggregation bounds).
    pub async fn get_min_raw_created_at_before(
        &self,
        cutoff_ts: i64,
    ) -> anyhow::Result<Option<i64>> {
        let row = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MIN(created_at) FROM system_history WHERE created_at < $1",
        )
        .bind(cutoff_ts)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Delete raw rows in [from_ts, to_ts).
    #[instrument(skip(self), fields(repo = "history", operation = "delete_raw_range"))]
    pub async fn delete_raw_range(&self, from_ts: i64, to_ts: i64) -> anyhow::Result<u64> {
        let r =
            sqlx::query("DELETE FROM system_history WHERE created_at >= $1 AND created_at < $2")
                .bind(from_ts)
                .bind(to_ts)
                .execute(&self.pool)
                .await?;
        Ok(r.rows_affected())
    }

    /// Aggregated snapshots in [from_ts, to_ts) for the given resolution. Order: ascending by created_at.
    #[instrument(
        skip(self),
        fields(repo = "history", operation = "get_aggregated_snapshots_by_time_range")
    )]
    pub async fn get_aggregated_snapshots_by_time_range(
        &self,
        from_ts: i64,
        to_ts: i64,
        resolution_seconds: i32,
    ) -> anyhow::Result<Vec<AggregatedSnapshot>> {
        let rows = sqlx::query(
            "SELECT created_at, resolution_seconds, cpu_load_avg, cpu_load_min, cpu_load_max,
                    memory_used_avg, memory_used_min, memory_used_max,
                    container_data, storage_data, network_data, system_data
             FROM system_history_aggregated
             WHERE created_at >= $1 AND created_at < $2 AND resolution_seconds = $3
             ORDER BY created_at ASC",
        )
        .bind(from_ts)
        .bind(to_ts)
        .bind(resolution_seconds)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(Self::parse_aggregated_row(&row)?);
        }
        Ok(out)
    }

    /// Minimum created_at in system_history_aggregated with created_at < cutoff_ts and given resolution.
    pub async fn get_min_aggregated_created_at_before(
        &self,
        cutoff_ts: i64,
        resolution_seconds: i32,
    ) -> anyhow::Result<Option<i64>> {
        let row = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MIN(created_at) FROM system_history_aggregated WHERE created_at < $1 AND resolution_seconds = $2",
        )
        .bind(cutoff_ts)
        .bind(resolution_seconds)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Delete aggregated rows in [from_ts, to_ts) for the given resolution.
    #[instrument(
        skip(self),
        fields(repo = "history", operation = "delete_aggregated_range")
    )]
    pub async fn delete_aggregated_range(
        &self,
        from_ts: i64,
        to_ts: i64,
        resolution_seconds: i32,
    ) -> anyhow::Result<u64> {
        let r = sqlx::query(
            "DELETE FROM system_history_aggregated WHERE created_at >= $1 AND created_at < $2 AND resolution_seconds = $3",
        )
        .bind(from_ts)
        .bind(to_ts)
        .bind(resolution_seconds)
        .execute(&self.pool)
        .await?;
        Ok(r.rows_affected())
    }

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

    /// Prune aggregated rows older than retention_days.
    #[instrument(
        skip(self),
        fields(repo = "history", operation = "prune_aggregated_old_data")
    )]
    pub async fn prune_aggregated_old_data(&self) -> anyhow::Result<u64> {
        let cutoff = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as i64)
            - self.retention_ms;
        let r = sqlx::query("DELETE FROM system_history_aggregated WHERE created_at < $1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected())
    }

    /// Reclaim space after deletes (run periodically after pruning).
    #[instrument(skip(self), fields(repo = "history", operation = "vacuum"))]
    pub async fn vacuum(&self) -> anyhow::Result<()> {
        sqlx::query("VACUUM").execute(&self.pool).await?;
        Ok(())
    }

    fn parse_aggregated_row(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<AggregatedSnapshot> {
        use sqlx::Row;
        let created_at: i64 = row.try_get("created_at")?;
        let resolution_seconds: i32 = row.try_get("resolution_seconds")?;
        let cpu_load_avg: f64 = row.try_get("cpu_load_avg")?;
        let cpu_load_min: f64 = row.try_get("cpu_load_min")?;
        let cpu_load_max: f64 = row.try_get("cpu_load_max")?;
        let memory_used_avg: i64 = row.try_get("memory_used_avg")?;
        let memory_used_min: i64 = row.try_get("memory_used_min")?;
        let memory_used_max: i64 = row.try_get("memory_used_max")?;
        let container_data: Vec<u8> = row.try_get("container_data")?;
        let storage_data: Vec<u8> = row.try_get("storage_data")?;
        let network_data: Vec<u8> = row.try_get("network_data")?;
        let system_data: Vec<u8> = row.try_get("system_data")?;

        let containers = deserialize_container_data(&container_data);
        let storage = deserialize_storage_data(&storage_data);
        let network = deserialize_network_data(&network_data);
        let system = wincode::deserialize(blob::blob_payload(
            &system_data,
            blob::BLOB_VERSION_SYSTEM_DYNAMIC,
        ))
        .unwrap_or_else(|e| {
            tracing::debug!(error = %e, "wincode deserialize aggregated system, using default");
            SystemStatsDynamic {
                uptime_secs: 0,
                process_count: 0,
                thread_count: 0,
                cpu_voltage: 0.0,
                fan_speeds: vec![],
            }
        });

        Ok(AggregatedSnapshot {
            created_at,
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

    fn parse_snapshot_row(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<FullSystemSnapshot> {
        use sqlx::Row;
        let created_at: i64 = row.try_get("created_at")?;
        let cpu_load: f64 = row.try_get("cpu_load")?;
        let memory_used: i64 = row.try_get("memory_used")?;
        let container_data: Vec<u8> = row.try_get("container_data")?;
        let storage_data: Vec<u8> = row.try_get("storage_data")?;
        let network_data: Vec<u8> = row.try_get("network_data")?;
        let system_data: Vec<u8> = row.try_get("system_data")?;

        let containers = deserialize_container_data(&container_data);
        let storage = deserialize_storage_data(&storage_data);
        let network = deserialize_network_data(&network_data);

        let system = match blob::blob_version(&system_data) {
            blob::BLOB_VERSION_SYSTEM_DYNAMIC => {
                match wincode::deserialize::<SystemStatsDynamic>(blob::blob_payload(
                    &system_data,
                    blob::BLOB_VERSION_SYSTEM_DYNAMIC,
                )) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::debug!(error = %e, "wincode deserialize system (dynamic), using default");
                        SystemStatsDynamic {
                            uptime_secs: 0,
                            process_count: 0,
                            thread_count: 0,
                            cpu_voltage: 0.0,
                            fan_speeds: vec![],
                        }
                    }
                }
            }
            _ => match wincode::deserialize::<SystemStats>(blob::blob_payload(
                &system_data,
                blob::BLOB_VERSION,
            )) {
                Ok(full) => SystemStatsDynamic {
                    uptime_secs: full.uptime_secs,
                    process_count: full.process_count,
                    thread_count: full.thread_count,
                    cpu_voltage: full.cpu_voltage,
                    fan_speeds: full.fan_speeds,
                },
                Err(e) => {
                    tracing::debug!(error = %e, "wincode deserialize system (legacy), using default");
                    SystemStatsDynamic {
                        uptime_secs: 0,
                        process_count: 0,
                        thread_count: 0,
                        cpu_voltage: 0.0,
                        fan_speeds: vec![],
                    }
                }
            },
        };

        Ok(FullSystemSnapshot {
            timestamp: created_at as u64,
            cpu: CpuStats {
                model: String::new(),
                physical_cores: 0,
                logical_cores: 0,
                usage_percent: cpu_load,
                temperature: 0.0,
            },
            ram: RamStats {
                total: 0,
                used: memory_used as u64,
                available: 0,
                usage_percent: 0.0,
            },
            containers,
            storage,
            network,
            system,
        })
    }
}

/// Deserialize container_data; on legacy/corrupt blob return empty vec and log.
fn deserialize_container_data(bytes: &[u8]) -> Vec<ContainerStats> {
    wincode::deserialize(blob::blob_payload(bytes, blob::BLOB_VERSION)).unwrap_or_else(|e| {
        tracing::debug!(error = %e, "wincode deserialize containers (legacy/corrupt), using empty");
        vec![]
    })
}

fn deserialize_storage_data(bytes: &[u8]) -> StorageStats {
    wincode::deserialize(blob::blob_payload(bytes, blob::BLOB_VERSION)).unwrap_or_else(|e| {
        tracing::debug!(error = %e, "wincode deserialize storage (legacy/corrupt), using empty");
        StorageStats {
            partitions: vec![],
            disks: vec![],
        }
    })
}

fn deserialize_network_data(bytes: &[u8]) -> NetworkStats {
    wincode::deserialize(blob::blob_payload(bytes, blob::BLOB_VERSION)).unwrap_or_else(|e| {
        tracing::debug!(error = %e, "wincode deserialize network (legacy/corrupt), using empty");
        NetworkStats { interfaces: vec![] }
    })
}

fn aggregated_to_snapshot(agg: AggregatedSnapshot) -> FullSystemSnapshot {
    FullSystemSnapshot {
        timestamp: agg.created_at as u64,
        cpu: CpuStats {
            model: String::new(),
            physical_cores: 0,
            logical_cores: 0,
            usage_percent: agg.cpu_load_avg,
            temperature: 0.0,
        },
        ram: RamStats {
            total: 0,
            used: agg.memory_used_avg as u64,
            available: 0,
            usage_percent: 0.0,
        },
        containers: agg.containers,
        storage: agg.storage,
        network: agg.network,
        system: agg.system,
    }
}

fn downsample_snapshots(
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
