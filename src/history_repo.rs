// SQLite history (same schema as Kotlin server).
// Uses sqlx for async + connection pooling. Data stored as BLOB (wincode, bincode-compatible).
// BLOB layout: [version: u8][payload]. Version 0 = legacy (no prefix); version 1 = current.
// This allows schema evolution: future versions can add V2 and migrate on read.
//
// Inspecting BLOB data: run `cargo run --example dump_history -- [DB_PATH] [LIMIT]` to print
// recent snapshots as JSON (e.g. `cargo run --example dump_history -- ./data/history.db 5`).
//
// Future schema migrations: run versioned migrations (e.g. sqlx-cli migrate) or check a
// schema_version table and run ALTER/CREATE as needed before opening the pool.

use crate::models::{CpuStats, FullSystemSnapshot, RamStats};
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

const SEVEN_DAYS_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// Current BLOB format version. Prefixed to all new blobs for schema evolution.
const BLOB_VERSION: u8 = 1;

/// Prepend version byte to serialized payload for schema evolution.
fn with_version_prefix(payload: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + payload.len());
    out.push(BLOB_VERSION);
    out.extend_from_slice(&payload);
    out
}

/// Strip version prefix if present; return slice to wincode payload (legacy or versioned).
fn blob_payload(bytes: &[u8]) -> &[u8] {
    if bytes.is_empty() {
        bytes
    } else if bytes[0] == BLOB_VERSION {
        &bytes[1..]
    } else {
        bytes
    }
}

pub struct HistoryRepo {
    pool: SqlitePool,
}

impl HistoryRepo {
    /// Connect to SQLite at `path`, create parent dir and DB if missing, enable WAL + pragmas.
    pub async fn connect(path: &str) -> anyhow::Result<Self> {
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}", path))?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_secs(5))
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);
        let pool = SqlitePoolOptions::new().connect_with(opts).await?;
        Ok(Self { pool })
    }

    /// Create tables if they don't exist. Single schema: BLOB columns (wincode).
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

        Ok(())
    }

    pub async fn save_snapshots(&self, snapshots: &[FullSystemSnapshot]) -> anyhow::Result<()> {
        if snapshots.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for s in snapshots {
            let container_data = with_version_prefix(
                wincode::serialize(&s.containers).map_err(|e| anyhow::anyhow!("wincode: {}", e))?,
            );
            let storage_data = with_version_prefix(
                wincode::serialize(&s.storage).map_err(|e| anyhow::anyhow!("wincode: {}", e))?,
            );
            let network_data = with_version_prefix(
                wincode::serialize(&s.network).map_err(|e| anyhow::anyhow!("wincode: {}", e))?,
            );
            let system_data = with_version_prefix(
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

    pub async fn prune_old_data(&self) -> anyhow::Result<()> {
        let cutoff = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as i64)
            - SEVEN_DAYS_MS;
        sqlx::query("DELETE FROM system_history WHERE created_at < $1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Fetch the most recent snapshots (for inspection/debug). Deserializes BLOB columns with wincode.
    pub async fn get_recent_snapshots(
        &self,
        limit: u32,
    ) -> anyhow::Result<Vec<FullSystemSnapshot>> {
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

            let containers = wincode::deserialize(blob_payload(&container_data))
                .map_err(|e| anyhow::anyhow!("wincode deserialize containers: {}", e))?;
            let storage = wincode::deserialize(blob_payload(&storage_data))
                .map_err(|e| anyhow::anyhow!("wincode deserialize storage: {}", e))?;
            let network = wincode::deserialize(blob_payload(&network_data))
                .map_err(|e| anyhow::anyhow!("wincode deserialize network: {}", e))?;
            let system = wincode::deserialize(blob_payload(&system_data))
                .map_err(|e| anyhow::anyhow!("wincode deserialize system: {}", e))?;

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
        Ok(out)
    }
}
