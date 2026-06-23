// HistoryRepo tests: connect, init, save, get_recent, prune, aggregation (range, save_aggregated, delete)

use homeserver::history_repo::{CURRENT_SCHEMA_VERSION, HistoryRepo};
use homeserver::models::*;
use sqlx::sqlite::SqlitePool;
use std::path::Path;
use tempfile::TempDir;

async fn schema_version_value(db_path: &Path) -> Option<i64> {
    let url = format!("sqlite:{}", db_path.display());
    let pool = SqlitePool::connect(&url).await.unwrap();
    let v = sqlx::query_scalar::<_, i64>("SELECT value FROM schema_version WHERE key = 'schema'")
        .fetch_optional(&pool)
        .await
        .unwrap();
    pool.close().await;
    v
}

fn minimal_system_info() -> SystemInfo {
    SystemInfo {
        os_family: "Linux".into(),
        os_manufacturer: String::new(),
        os_version: String::new(),
        system_manufacturer: String::new(),
        system_model: String::new(),
        processor_name: String::new(),
    }
}

fn minimal_snapshot(timestamp: u64) -> FullSystemSnapshot {
    FullSystemSnapshot {
        timestamp,
        cpu: CpuStats {
            model: "test".into(),
            physical_cores: 1,
            logical_cores: 2,
            usage_percent: 10.0,
            temperature: 0.0,
            core_usages: vec![],
        },
        ram: RamStats {
            total: 1024,
            used: 512,
            available: 512,
            usage_percent: 50.0,
            swap_total: 0,
            swap_used: 0,
            swap_free: 0,
        },
        containers: vec![],
        storage: StorageStats {
            partitions: vec![],
            disks: vec![],
        },
        network: NetworkStats { interfaces: vec![] },
        system: SystemStatsDynamic {
            uptime_secs: 0,
            process_count: 0,
            thread_count: 0,
            load_avg_1: 0.0,
            load_avg_5: 0.0,
            load_avg_15: 0.0,
        },
    }
}

#[tokio::test]
async fn history_repo_connect_and_init() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history.db");
    let path_str = path.to_str().unwrap();

    let repo = HistoryRepo::connect(path_str, 3).await.unwrap();
    repo.init().await.unwrap();
    // Second init is no-op (IF NOT EXISTS)
    repo.init().await.unwrap();
}

#[tokio::test]
async fn history_repo_save_and_get_recent() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history.db");
    let path_str = path.to_str().unwrap();

    let repo = HistoryRepo::connect(path_str, 3).await.unwrap();
    repo.init().await.unwrap();

    let snapshots = vec![
        minimal_snapshot(1000),
        minimal_snapshot(2000),
        minimal_snapshot(3000),
    ];
    repo.save_snapshots(&snapshots, &minimal_system_info())
        .await
        .unwrap();

    let (_info, recent) = repo.get_recent_snapshots(10).await.unwrap();
    assert_eq!(recent.len(), 3);
    assert_eq!(recent[0].timestamp, 1000);
    assert_eq!(recent[1].timestamp, 2000);
    assert_eq!(recent[2].timestamp, 3000);
    assert_eq!(recent[0].cpu.usage_percent, 10.0);
    assert_eq!(recent[0].ram.used, 512);

    let (_info2, limited) = repo.get_recent_snapshots(2).await.unwrap();
    assert_eq!(limited.len(), 2);
    assert_eq!(limited[0].timestamp, 2000);
    assert_eq!(limited[1].timestamp, 3000);
}

#[tokio::test]
async fn history_repo_save_empty_no_op() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history.db");
    let path_str = path.to_str().unwrap();

    let repo = HistoryRepo::connect(path_str, 3).await.unwrap();
    repo.init().await.unwrap();
    repo.save_snapshots(&[], &minimal_system_info())
        .await
        .unwrap();

    let (_info, recent) = repo.get_recent_snapshots(10).await.unwrap();
    assert!(recent.is_empty());
}

#[tokio::test]
async fn history_repo_prune_old_data() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history.db");
    let path_str = path.to_str().unwrap();

    let repo = HistoryRepo::connect(path_str, 3).await.unwrap();
    repo.init().await.unwrap();

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let old_ms = now_ms - (4 * 24 * 60 * 60 * 1000); // 4 days ago (will be pruned with 3 day retention)

    repo.save_snapshots(
        &[minimal_snapshot(old_ms), minimal_snapshot(now_ms)],
        &minimal_system_info(),
    )
    .await
    .unwrap();
    let (_info, recent_before) = repo.get_recent_snapshots(10).await.unwrap();
    assert_eq!(recent_before.len(), 2);

    repo.prune_old_data().await.unwrap();
    let (_info2, recent_after) = repo.get_recent_snapshots(10).await.unwrap();
    assert_eq!(recent_after.len(), 1);
    assert_eq!(recent_after[0].timestamp, now_ms);
}

#[tokio::test]
async fn schema_version_written_on_fresh_db() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history.db");

    let repo = HistoryRepo::connect(path.to_str().unwrap(), 3)
        .await
        .unwrap();
    repo.init().await.unwrap();

    let v = schema_version_value(&path)
        .await
        .expect("schema row present");
    assert_eq!(v, CURRENT_SCHEMA_VERSION as i64);
}

#[tokio::test]
async fn schema_version_unchanged_on_reinit() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history.db");

    let repo = HistoryRepo::connect(path.to_str().unwrap(), 3)
        .await
        .unwrap();
    repo.init().await.unwrap();
    repo.init().await.unwrap();

    let v = schema_version_value(&path)
        .await
        .expect("schema row present");
    assert_eq!(v, CURRENT_SCHEMA_VERSION as i64);
}

#[tokio::test]
async fn schema_version_mismatch_purges_tables() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history.db");
    let path_str = path.to_str().unwrap();

    let repo = HistoryRepo::connect(path_str, 3).await.unwrap();
    repo.init().await.unwrap();

    let pool = SqlitePool::connect(&format!("sqlite:{}", path.display()))
        .await
        .unwrap();
    sqlx::query("UPDATE schema_version SET value = 0 WHERE key = 'schema'")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    repo.save_snapshots(&[minimal_snapshot(999)], &minimal_system_info())
        .await
        .unwrap();
    let (_info_before, recent_before) = repo.get_recent_snapshots(10).await.unwrap();
    assert_eq!(recent_before.len(), 1);

    repo.init().await.unwrap();

    let (_info_after, recent_after) = repo.get_recent_snapshots(10).await.unwrap();
    assert!(recent_after.is_empty());

    let v = schema_version_value(&path)
        .await
        .expect("schema row present");
    assert_eq!(v, CURRENT_SCHEMA_VERSION as i64);
}

#[tokio::test]
async fn schema_version_missing_row_with_existing_history_purges() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history.db");
    let path_str = path.to_str().unwrap();

    let repo = HistoryRepo::connect(path_str, 3).await.unwrap();
    repo.init().await.unwrap();
    repo.save_snapshots(&[minimal_snapshot(111)], &minimal_system_info())
        .await
        .unwrap();

    let pool = SqlitePool::connect(&format!("sqlite:{}", path.display()))
        .await
        .unwrap();
    sqlx::query("DELETE FROM schema_version WHERE key = 'schema'")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    repo.init().await.unwrap();

    let (_info, recent) = repo.get_recent_snapshots(10).await.unwrap();
    assert!(
        recent.is_empty(),
        "pre-versioning DB without schema row must purge incompatible history"
    );
    let v = schema_version_value(&path)
        .await
        .expect("schema row present");
    assert_eq!(v, CURRENT_SCHEMA_VERSION as i64);
}
