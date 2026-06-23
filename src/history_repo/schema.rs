// Pool connection, schema version, DDL for raw tables.

use super::{CURRENT_SCHEMA_VERSION, HistoryRepo};
use crate::history_repo::aggregation;
use std::path::Path;
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

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
            Some(found) => {
                tracing::warn!(
                    "schema version mismatch: found {}, expected {}; purging history",
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
}
