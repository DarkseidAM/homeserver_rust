// HistoryRepo tests: connect, init, save, get_recent, prune

use homeserver::history_repo::HistoryRepo;
use homeserver::models::*;
use tempfile::TempDir;

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
        network: NetworkStats {
            interfaces: vec![],
        },
        system: SystemStats {
            os_family: "Linux".into(),
            os_manufacturer: String::new(),
            os_version: String::new(),
            system_manufacturer: String::new(),
            system_model: String::new(),
            processor_name: String::new(),
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

    let repo = HistoryRepo::connect(path_str).await.unwrap();
    repo.init().await.unwrap();
    // Second init is no-op (IF NOT EXISTS)
    repo.init().await.unwrap();
}

#[tokio::test]
async fn history_repo_save_and_get_recent() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history.db");
    let path_str = path.to_str().unwrap();

    let repo = HistoryRepo::connect(path_str).await.unwrap();
    repo.init().await.unwrap();

    let snapshots = vec![
        minimal_snapshot(1000),
        minimal_snapshot(2000),
        minimal_snapshot(3000),
    ];
    repo.save_snapshots(&snapshots).await.unwrap();

    let recent = repo.get_recent_snapshots(10).await.unwrap();
    assert_eq!(recent.len(), 3);
    assert_eq!(recent[0].timestamp, 1000);
    assert_eq!(recent[1].timestamp, 2000);
    assert_eq!(recent[2].timestamp, 3000);
    assert_eq!(recent[0].cpu.usage_percent, 10.0);
    assert_eq!(recent[0].ram.used, 512);

    let limited = repo.get_recent_snapshots(2).await.unwrap();
    assert_eq!(limited.len(), 2);
    assert_eq!(limited[0].timestamp, 2000);
    assert_eq!(limited[1].timestamp, 3000);
}

#[tokio::test]
async fn history_repo_save_empty_no_op() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history.db");
    let path_str = path.to_str().unwrap();

    let repo = HistoryRepo::connect(path_str).await.unwrap();
    repo.init().await.unwrap();
    repo.save_snapshots(&[]).await.unwrap();

    let recent = repo.get_recent_snapshots(10).await.unwrap();
    assert!(recent.is_empty());
}

#[tokio::test]
async fn history_repo_prune_old_data() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history.db");
    let path_str = path.to_str().unwrap();

    let repo = HistoryRepo::connect(path_str).await.unwrap();
    repo.init().await.unwrap();

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let old_ms = now_ms - (8 * 24 * 60 * 60 * 1000); // 8 days ago

    repo.save_snapshots(&[minimal_snapshot(old_ms), minimal_snapshot(now_ms)])
        .await
        .unwrap();
    let recent_before = repo.get_recent_snapshots(10).await.unwrap();
    assert_eq!(recent_before.len(), 2);

    repo.prune_old_data().await.unwrap();
    let recent_after = repo.get_recent_snapshots(10).await.unwrap();
    assert_eq!(recent_after.len(), 1);
    assert_eq!(recent_after[0].timestamp, now_ms);
}
