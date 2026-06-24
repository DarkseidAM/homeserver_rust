// Raw `system_history` + `system_info` reads and writes.

use crate::history_repo::HistoryRepo;
use crate::history_repo::blob;
use crate::history_repo::history_merge::{
    deserialize_container_data, deserialize_cpu_data, deserialize_network_data,
    deserialize_ram_data, deserialize_storage_data,
};
use crate::models::{FullSystemSnapshot, SystemInfo, SystemStats, SystemStatsDynamic};
use sqlx::Row;
use tracing::instrument;

impl HistoryRepo {
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
            let cpu_data = blob::with_version_prefix(
                blob::BLOB_VERSION,
                wincode::serialize(&s.cpu).map_err(|e| anyhow::anyhow!("wincode: {}", e))?,
            );
            let ram_data = blob::with_version_prefix(
                blob::BLOB_VERSION,
                wincode::serialize(&s.ram).map_err(|e| anyhow::anyhow!("wincode: {}", e))?,
            );
            sqlx::query(
                "INSERT INTO system_history (created_at, cpu_load, memory_used, container_data, storage_data, network_data, system_data, cpu_data, ram_data) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            )
            .bind(s.timestamp as i64)
            .bind(s.cpu.usage_percent)
            .bind(s.ram.used as i64)
            .bind(&container_data)
            .bind(&storage_data)
            .bind(&network_data)
            .bind(&system_data)
            .bind(&cpu_data)
            .bind(&ram_data)
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
            "SELECT created_at, cpu_load, memory_used, container_data, storage_data, network_data, system_data, cpu_data, ram_data
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
            "SELECT created_at, cpu_load, memory_used, container_data, storage_data, network_data, system_data, cpu_data, ram_data
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

    fn parse_snapshot_row(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<FullSystemSnapshot> {
        let created_at: i64 = row.try_get("created_at")?;
        let cpu_load: f64 = row.try_get("cpu_load")?;
        let memory_used: i64 = row.try_get("memory_used")?;
        let container_data: Vec<u8> = row.try_get("container_data")?;
        let storage_data: Vec<u8> = row.try_get("storage_data")?;
        let network_data: Vec<u8> = row.try_get("network_data")?;
        let system_data: Vec<u8> = row.try_get("system_data")?;
        // Nullable on rows written before schema v3 (full CPU/RAM persistence).
        let cpu_data: Option<Vec<u8>> = row.try_get("cpu_data")?;
        let ram_data: Option<Vec<u8>> = row.try_get("ram_data")?;

        let containers = deserialize_container_data(&container_data);
        let storage = deserialize_storage_data(&storage_data);
        let network = deserialize_network_data(&network_data);
        let cpu = deserialize_cpu_data(cpu_data.as_deref(), cpu_load);
        let ram = deserialize_ram_data(ram_data.as_deref(), memory_used as u64);

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
                            load_avg_1: 0.0,
                            load_avg_5: 0.0,
                            load_avg_15: 0.0,
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
                    load_avg_1: 0.0,
                    load_avg_5: 0.0,
                    load_avg_15: 0.0,
                },
                Err(e) => {
                    tracing::debug!(error = %e, "wincode deserialize system (legacy), using default");
                    SystemStatsDynamic {
                        uptime_secs: 0,
                        process_count: 0,
                        thread_count: 0,
                        load_avg_1: 0.0,
                        load_avg_5: 0.0,
                        load_avg_15: 0.0,
                    }
                }
            },
        };

        Ok(FullSystemSnapshot {
            timestamp: created_at as u64,
            cpu,
            ram,
            containers,
            storage,
            network,
            system,
        })
    }
}
