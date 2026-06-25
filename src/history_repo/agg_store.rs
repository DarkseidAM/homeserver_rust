// `system_history_aggregated` reads, writes, and retention prune.

use crate::history_repo::HistoryRepo;
use crate::history_repo::blob;
use crate::history_repo::history_merge::{
    deserialize_container_data, deserialize_cpu_data, deserialize_network_data,
    deserialize_ram_data, deserialize_storage_data,
};
use crate::models::{AggregatedSnapshot, SystemStatsDynamic};
use sqlx::Row;
use tracing::instrument;

impl HistoryRepo {
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
        let cpu_data = blob::with_version_prefix(
            blob::BLOB_VERSION,
            wincode::serialize(&agg.cpu).map_err(|e| anyhow::anyhow!("wincode: {}", e))?,
        );
        let ram_data = blob::with_version_prefix(
            blob::BLOB_VERSION,
            wincode::serialize(&agg.ram).map_err(|e| anyhow::anyhow!("wincode: {}", e))?,
        );

        sqlx::query(
            r#"
            INSERT INTO system_history_aggregated
            (created_at, resolution_seconds, cpu_load_avg, cpu_load_min, cpu_load_max,
             memory_used_avg, memory_used_min, memory_used_max,
             container_data, storage_data, network_data, system_data, cpu_data, ram_data)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
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
        .bind(&cpu_data)
        .bind(&ram_data)
        .execute(&self.pool)
        .await?;

        Ok(())
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
                    container_data, storage_data, network_data, system_data, cpu_data, ram_data
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

    fn parse_aggregated_row(row: &sqlx::sqlite::SqliteRow) -> anyhow::Result<AggregatedSnapshot> {
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
        // Nullable on rows written before schema v3.
        let cpu_data: Option<Vec<u8>> = row.try_get("cpu_data")?;
        let ram_data: Option<Vec<u8>> = row.try_get("ram_data")?;

        let containers = deserialize_container_data(&container_data);
        let storage = deserialize_storage_data(&storage_data);
        let network = deserialize_network_data(&network_data);
        let cpu = deserialize_cpu_data(cpu_data.as_deref(), cpu_load_avg);
        let ram = deserialize_ram_data(ram_data.as_deref(), memory_used_avg as u64);
        let system = wincode::deserialize(blob::blob_payload(
            &system_data,
            blob::BLOB_VERSION_SYSTEM_DYNAMIC,
        ))
        .unwrap_or_else(|e| {
            tracing::debug!(error = %e, "wincode deserialize aggregated system, using default");
            SystemStatsDynamic::default()
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
            cpu,
            ram,
            containers,
            storage,
            network,
            system,
        })
    }
}
