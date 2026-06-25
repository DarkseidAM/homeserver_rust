// Pool connection, schema version, DDL for raw tables.

use super::{CURRENT_SCHEMA_VERSION, HistoryRepo};
use crate::history_repo::aggregation;
use std::path::Path;
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

/// Ordered, additive forward migrations. Entry `(v, statements)` migrates schema `v` → `v + 1`.
/// Only data-preserving DDL (e.g. `ALTER TABLE ... ADD COLUMN`) belongs here.
/// v2 → v3: add nullable `cpu_data` / `ram_data` blobs so full CPU/RAM detail is persisted;
/// old rows keep NULL and are read via the scalar fallback.
const MIGRATIONS: &[(u32, &[&str])] = &[
    (
        2,
        &[
            "ALTER TABLE system_history ADD COLUMN cpu_data BLOB",
            "ALTER TABLE system_history ADD COLUMN ram_data BLOB",
            "ALTER TABLE system_history_aggregated ADD COLUMN cpu_data BLOB",
            "ALTER TABLE system_history_aggregated ADD COLUMN ram_data BLOB",
        ],
    ),
    // v3 → v4: persist GPU metrics. Nullable; rows without it read as an empty GPU list.
    (
        3,
        &[
            "ALTER TABLE system_history ADD COLUMN gpu_data BLOB",
            "ALTER TABLE system_history_aggregated ADD COLUMN gpu_data BLOB",
        ],
    ),
];

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

    async fn drop_history_user_tables(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ) -> anyhow::Result<()> {
        sqlx::query("DROP TABLE IF EXISTS system_history")
            .execute(&mut **tx)
            .await?;
        sqlx::query("DROP TABLE IF EXISTS system_history_aggregated")
            .execute(&mut **tx)
            .await?;
        sqlx::query("DROP TABLE IF EXISTS system_info")
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    async fn ensure_schema_version(&self) -> anyhow::Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schema_version (key TEXT PRIMARY KEY, value INTEGER NOT NULL)",
        )
        .execute(&self.pool)
        .await?;

        let row: Option<i64> =
            sqlx::query_scalar("SELECT value FROM schema_version WHERE key = 'schema'")
                .fetch_optional(&self.pool)
                .await?;

        match row {
            None => {
                let legacy_tables: i64 = sqlx::query_scalar(
                    r#"SELECT COUNT(*) FROM sqlite_master
                       WHERE type = 'table'
                         AND name IN ('system_history', 'system_info', 'system_history_aggregated')"#,
                )
                .fetch_one(&self.pool)
                .await?;

                if legacy_tables > 0 {
                    tracing::warn!(
                        "schema version row missing but history tables present; purging history"
                    );
                    let mut tx = self.pool.begin().await?;
                    Self::drop_history_user_tables(&mut tx).await?;
                    sqlx::query(
                        r#"INSERT INTO schema_version (key, value) VALUES ('schema', $1)
                           ON CONFLICT(key) DO UPDATE SET value = excluded.value"#,
                    )
                    .bind(i64::from(CURRENT_SCHEMA_VERSION))
                    .execute(&mut *tx)
                    .await?;
                    tx.commit().await?;
                } else {
                    sqlx::query(
                        r#"INSERT INTO schema_version (key, value) VALUES ('schema', $1)
                           ON CONFLICT(key) DO NOTHING"#,
                    )
                    .bind(i64::from(CURRENT_SCHEMA_VERSION))
                    .execute(&self.pool)
                    .await?;
                }
            }
            Some(v) if v == i64::from(CURRENT_SCHEMA_VERSION) => {}
            Some(found) if found > 0 && found < i64::from(CURRENT_SCHEMA_VERSION) => {
                // Forward, data-preserving migration.
                self.run_migrations(found as u32).await?;
            }
            Some(found) => {
                // Downgrade or unknown future version: no safe path, purge.
                tracing::warn!(
                    "schema version {} newer than supported {} (downgrade?); purging history",
                    found,
                    CURRENT_SCHEMA_VERSION
                );
                let mut tx = self.pool.begin().await?;
                Self::drop_history_user_tables(&mut tx).await?;
                sqlx::query("UPDATE schema_version SET value = $1 WHERE key = 'schema'")
                    .bind(i64::from(CURRENT_SCHEMA_VERSION))
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
            }
        }

        Ok(())
    }

    /// Apply ordered, additive, data-preserving migrations from `from_version` up to
    /// `CURRENT_SCHEMA_VERSION`. Each step runs in its own transaction (SQLite DDL is
    /// transactional, so a crash mid-step rolls back cleanly and is retried next start).
    /// If a step has no registered migration, falls back to a destructive purge.
    async fn run_migrations(&self, from_version: u32) -> anyhow::Result<()> {
        let mut version = from_version;
        while version < CURRENT_SCHEMA_VERSION {
            let Some((_, statements)) = MIGRATIONS.iter().find(|(v, _)| *v == version) else {
                tracing::warn!(
                    "no migration registered from schema v{}; purging history",
                    version
                );
                let mut tx = self.pool.begin().await?;
                Self::drop_history_user_tables(&mut tx).await?;
                sqlx::query("UPDATE schema_version SET value = $1 WHERE key = 'schema'")
                    .bind(i64::from(CURRENT_SCHEMA_VERSION))
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                return Ok(());
            };
            let mut tx = self.pool.begin().await?;
            for stmt in *statements {
                // `*stmt` is a `&'static str` from the MIGRATIONS table (not user input).
                sqlx::query(*stmt).execute(&mut *tx).await?;
            }
            sqlx::query("UPDATE schema_version SET value = $1 WHERE key = 'schema'")
                .bind(i64::from(version + 1))
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            tracing::info!("migrated history schema v{} -> v{}", version, version + 1);
            version += 1;
        }
        Ok(())
    }

    pub async fn init(&self) -> anyhow::Result<()> {
        self.ensure_schema_version().await?;

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
                system_data BLOB NOT NULL,
                cpu_data BLOB,
                ram_data BLOB,
                gpu_data BLOB
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
}
