// SQLite history. system_info table stores static SystemInfo once; merge when loading.

mod blob;

use crate::models::{
    CpuStats, FullSystemSnapshot, RamStats, SystemInfo, SystemStats, SystemStatsDynamic,
};
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
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
            let created_at: i64 = row.try_get("created_at")?;
            let cpu_load: f64 = row.try_get("cpu_load")?;
            let memory_used: i64 = row.try_get("memory_used")?;
            let container_data: Vec<u8> = row.try_get("container_data")?;
            let storage_data: Vec<u8> = row.try_get("storage_data")?;
            let network_data: Vec<u8> = row.try_get("network_data")?;
            let system_data: Vec<u8> = row.try_get("system_data")?;

            let containers =
                wincode::deserialize(blob::blob_payload(&container_data, blob::BLOB_VERSION))
                    .map_err(|e| anyhow::anyhow!("wincode deserialize containers: {}", e))?;
            let storage =
                wincode::deserialize(blob::blob_payload(&storage_data, blob::BLOB_VERSION))
                    .map_err(|e| anyhow::anyhow!("wincode deserialize storage: {}", e))?;
            let network =
                wincode::deserialize(blob::blob_payload(&network_data, blob::BLOB_VERSION))
                    .map_err(|e| anyhow::anyhow!("wincode deserialize network: {}", e))?;

            let system = match blob::blob_version(&system_data) {
                blob::BLOB_VERSION_SYSTEM_DYNAMIC => wincode::deserialize(blob::blob_payload(
                    &system_data,
                    blob::BLOB_VERSION_SYSTEM_DYNAMIC,
                ))
                .map_err(|e| anyhow::anyhow!("wincode deserialize system (dynamic): {}", e))?,
                _ => {
                    let full: SystemStats =
                        wincode::deserialize(blob::blob_payload(&system_data, blob::BLOB_VERSION))
                            .map_err(|e| {
                                anyhow::anyhow!("wincode deserialize system (legacy): {}", e)
                            })?;
                    SystemStatsDynamic {
                        uptime_secs: full.uptime_secs,
                        process_count: full.process_count,
                        thread_count: full.thread_count,
                        cpu_voltage: full.cpu_voltage,
                        fan_speeds: full.fan_speeds,
                    }
                }
            };

            out.push(FullSystemSnapshot {
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
            });
        }
        out.reverse();
        Ok((stored_info, out))
    }
}
