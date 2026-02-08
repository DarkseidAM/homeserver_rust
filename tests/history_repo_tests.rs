// HistoryRepo tests: connect, init, save, get_recent, prune, aggregation (range, save_aggregated, delete)

use homeserver::history_repo::HistoryRepo;
use homeserver::models::*;
use tempfile::TempDir;

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
        },
        ram: RamStats {
            total: 1024,
            used: 512,
            available: 512,
            usage_percent: 50.0,
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
            cpu_voltage: 0.0,
            fan_speeds: vec![],
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

fn minimal_aggregated_snapshot(created_at: i64) -> AggregatedSnapshot {
    AggregatedSnapshot {
        created_at,
        resolution_seconds: 60,
        cpu_load_avg: 10.0,
        cpu_load_min: 5.0,
        cpu_load_max: 15.0,
        memory_used_avg: 512,
        memory_used_min: 256,
        memory_used_max: 768,
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
            cpu_voltage: 0.0,
            fan_speeds: vec![],
        },
    }
}

#[tokio::test]
async fn history_repo_init_creates_aggregated_table() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history.db");
    let path_str = path.to_str().unwrap();

    let repo = HistoryRepo::connect(path_str, 3).await.unwrap();
    repo.init().await.unwrap();

    let agg = minimal_aggregated_snapshot(60_000);
    repo.save_aggregated_snapshot(&agg).await.unwrap();
}

#[tokio::test]
async fn history_repo_get_raw_snapshots_by_time_range() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history.db");
    let path_str = path.to_str().unwrap();

    let repo = HistoryRepo::connect(path_str, 3).await.unwrap();
    repo.init().await.unwrap();

    let snapshots = vec![
        minimal_snapshot(1000),
        minimal_snapshot(2000),
        minimal_snapshot(3000),
        minimal_snapshot(4000),
    ];
    repo.save_snapshots(&snapshots, &minimal_system_info())
        .await
        .unwrap();

    let range = repo
        .get_raw_snapshots_by_time_range(2000, 4000)
        .await
        .unwrap();
    assert_eq!(range.len(), 2);
    assert_eq!(range[0].timestamp, 2000);
    assert_eq!(range[1].timestamp, 3000);

    let empty = repo
        .get_raw_snapshots_by_time_range(5000, 6000)
        .await
        .unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn history_repo_get_min_raw_created_at_before() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history.db");
    let path_str = path.to_str().unwrap();

    let repo = HistoryRepo::connect(path_str, 3).await.unwrap();
    repo.init().await.unwrap();

    let before = repo.get_min_raw_created_at_before(1000).await.unwrap();
    assert!(before.is_none());

    repo.save_snapshots(
        &[minimal_snapshot(1000), minimal_snapshot(2000)],
        &minimal_system_info(),
    )
    .await
    .unwrap();

    let min_before_5000 = repo.get_min_raw_created_at_before(5000).await.unwrap();
    assert_eq!(min_before_5000, Some(1000));

    let min_before_1500 = repo.get_min_raw_created_at_before(1500).await.unwrap();
    assert_eq!(min_before_1500, Some(1000));
}

#[tokio::test]
async fn history_repo_delete_raw_range() {
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

    let deleted = repo.delete_raw_range(2000, 3000).await.unwrap();
    assert_eq!(deleted, 1);

    let (_info, recent) = repo.get_recent_snapshots(10).await.unwrap();
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].timestamp, 1000);
    assert_eq!(recent[1].timestamp, 3000);
}

#[tokio::test]
async fn history_repo_get_aggregated_snapshots_by_time_range() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history.db");
    let path_str = path.to_str().unwrap();

    let repo = HistoryRepo::connect(path_str, 3).await.unwrap();
    repo.init().await.unwrap();

    let aggs = vec![
        minimal_aggregated_snapshot(60_000),
        minimal_aggregated_snapshot(120_000),
        minimal_aggregated_snapshot(180_000),
    ];
    for agg in &aggs {
        repo.save_aggregated_snapshot(agg).await.unwrap();
    }

    let range = repo
        .get_aggregated_snapshots_by_time_range(120_000, 180_000, 60)
        .await
        .unwrap();
    assert_eq!(range.len(), 1);
    assert_eq!(range[0].created_at, 120_000);
    assert_eq!(range[0].resolution_seconds, 60);

    let empty = repo
        .get_aggregated_snapshots_by_time_range(200_000, 300_000, 60)
        .await
        .unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn history_repo_delete_aggregated_range() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history.db");
    let path_str = path.to_str().unwrap();

    let repo = HistoryRepo::connect(path_str, 3).await.unwrap();
    repo.init().await.unwrap();

    for ts in [60_000, 120_000, 180_000] {
        repo.save_aggregated_snapshot(&minimal_aggregated_snapshot(ts))
            .await
            .unwrap();
    }

    let deleted = repo
        .delete_aggregated_range(120_000, 180_000, 60)
        .await
        .unwrap();
    assert_eq!(deleted, 1);

    let remaining = repo
        .get_aggregated_snapshots_by_time_range(0, 300_000, 60)
        .await
        .unwrap();
    assert_eq!(remaining.len(), 2);
    assert_eq!(remaining[0].created_at, 60_000);
    assert_eq!(remaining[1].created_at, 180_000);
}
