// SQLite history. system_info table stores static SystemInfo once; merge when loading.

mod agg_store;
pub mod aggregation;
mod blob;
mod history_merge;
mod raw;
mod schema;

pub const CURRENT_SCHEMA_VERSION: u32 = 2;

use sqlx::sqlite::SqlitePool;

pub struct HistoryRepo {
    pub(in crate::history_repo) pool: SqlitePool,
    pub(in crate::history_repo) retention_ms: i64,
}
