// Schema migration tests: a pre-v3 database must be upgraded in place (additive ALTER),
// preserving existing rows, and new writes must carry full CPU/RAM detail.

use homeserver::history_repo::HistoryRepo;
use homeserver::models::*;
use sqlx::Row;
use sqlx::sqlite::SqliteConnectOptions;
use std::str::FromStr;
use tempfile::TempDir;

/// Builds a schema-v2 database by hand: the old table shapes (no cpu_data/ram_data columns),
/// schema_version = 2, and one legacy raw row. Returns the db path (kept alive by `dir`).
async fn make_v2_db(path: &str) {
    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}", path))
        .unwrap()
        .create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(opts).await.unwrap();

    sqlx::query("CREATE TABLE schema_version (key TEXT PRIMARY KEY, value INTEGER NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO schema_version (key, value) VALUES ('schema', 2)")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        r#"CREATE TABLE system_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at INTEGER NOT NULL,
            cpu_load REAL NOT NULL,
            memory_used INTEGER NOT NULL,
            container_data BLOB NOT NULL,
            storage_data BLOB NOT NULL,
            network_data BLOB NOT NULL,
            system_data BLOB NOT NULL
        )"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE system_info (id INTEGER PRIMARY KEY CHECK (id = 1), data BLOB NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"CREATE TABLE system_history_aggregated (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at INTEGER NOT NULL,
            resolution_seconds INTEGER NOT NULL,
            cpu_load_avg REAL NOT NULL,
            cpu_load_min REAL,
            cpu_load_max REAL,
            memory_used_avg INTEGER NOT NULL,
            memory_used_min INTEGER,
            memory_used_max INTEGER,
            container_data BLOB NOT NULL,
            storage_data BLOB NOT NULL,
            network_data BLOB NOT NULL,
            system_data BLOB NOT NULL
        )"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // One legacy raw row. Blob columns hold only a version byte; the deserialize helpers
    // tolerate non-decodable payloads and fall back to empty/default values.
    let stub: &[u8] = &[1u8];
    sqlx::query(
        "INSERT INTO system_history (created_at, cpu_load, memory_used, container_data, storage_data, network_data, system_data)
         VALUES (1700000000000, 42.5, 123456, $1, $1, $1, $1)",
    )
    .bind(stub)
    .execute(&pool)
    .await
    .unwrap();

    pool.close().await;
}

#[tokio::test]
async fn migrates_v2_to_v3_preserving_rows() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v2.db");
    let path_str = path.to_str().unwrap();
    make_v2_db(path_str).await;

    // Connecting + init must migrate (not purge) and bump the schema version to current.
    let repo = HistoryRepo::connect(path_str, 3).await.unwrap();
    repo.init().await.unwrap();

    // Verify the schema version was advanced and the new columns exist.
    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}", path_str)).unwrap();
    let pool = sqlx::SqlitePool::connect_with(opts).await.unwrap();
    let version: i64 = sqlx::query_scalar("SELECT value FROM schema_version WHERE key = 'schema'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(version, 5, "schema version migrated to current");
    // v3 cpu_data/ram_data, v4 gpu_data, v5 smart_data columns now present on both tables.
    sqlx::query("SELECT cpu_data, ram_data, gpu_data, smart_data FROM system_history LIMIT 1")
        .fetch_optional(&pool)
        .await
        .expect("v3/v4/v5 columns exist on system_history");
    sqlx::query(
        "SELECT cpu_data, ram_data, gpu_data, smart_data FROM system_history_aggregated LIMIT 1",
    )
    .fetch_optional(&pool)
    .await
    .expect("v3/v4/v5 columns exist on aggregated table");

    // The legacy row survived the migration (no purge).
    let count: i64 = sqlx::query("SELECT COUNT(*) AS c FROM system_history")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("c");
    assert_eq!(count, 1, "legacy row preserved across migration");
    pool.close().await;

    // The legacy row reads back with its scalar cpu_load surfaced via the CPU fallback.
    let (_info, snaps) = repo.get_recent_snapshots(10).await.unwrap();
    assert_eq!(snaps.len(), 1);
    assert!((snaps[0].cpu.usage_percent - 42.5).abs() < 0.001);
    assert_eq!(snaps[0].ram.used, 123456);
}

#[tokio::test]
async fn new_writes_persist_full_cpu_ram_detail() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let repo = HistoryRepo::connect(path.to_str().unwrap(), 3)
        .await
        .unwrap();
    repo.init().await.unwrap();

    let info = SystemInfo {
        os_family: "Linux".into(),
        os_manufacturer: String::new(),
        os_version: String::new(),
        system_manufacturer: String::new(),
        system_model: String::new(),
        processor_name: String::new(),
    };
    let snap = FullSystemSnapshot {
        timestamp: 1700000001000,
        cpu: CpuStats {
            model: "Test CPU".into(),
            physical_cores: 4,
            logical_cores: 8,
            usage_percent: 33.3,
            temperature: 61.0,
            core_usages: vec![10.0, 20.0, 30.0, 40.0],
        },
        ram: RamStats {
            total: 16_000,
            used: 4_000,
            available: 12_000,
            usage_percent: 25.0,
            swap_total: 2_000,
            swap_used: 100,
            swap_free: 1_900,
        },
        containers: vec![],
        storage: StorageStats::default(),
        network: NetworkStats::default(),
        system: SystemStatsDynamic::default(),
        gpus: vec![],
        smart: vec![],
    };
    repo.save_snapshots(std::slice::from_ref(&snap), &info)
        .await
        .unwrap();

    let (_info, snaps) = repo.get_recent_snapshots(10).await.unwrap();
    assert_eq!(snaps.len(), 1);
    let cpu = &snaps[0].cpu;
    assert_eq!(cpu.model, "Test CPU");
    assert_eq!(cpu.logical_cores, 8);
    assert!((cpu.temperature - 61.0).abs() < 0.001);
    assert_eq!(cpu.core_usages, vec![10.0, 20.0, 30.0, 40.0]);
    let ram = &snaps[0].ram;
    assert_eq!(ram.total, 16_000);
    assert_eq!(ram.swap_total, 2_000);
    assert_eq!(ram.available, 12_000);
}
