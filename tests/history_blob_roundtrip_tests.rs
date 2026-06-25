// Verifies GPU + SMART values survive a full save -> read round-trip through HistoryRepo
// (both the raw and aggregated tables), not just the in-memory wincode round-trip.

use homeserver::history_repo::HistoryRepo;
use homeserver::models::*;
use tempfile::TempDir;

fn gpu() -> GpuStats {
    GpuStats {
        index: 0,
        vendor: "nvidia".into(),
        name: "Test GPU".into(),
        utilization_percent: 77.0,
        memory_used_bytes: 2048,
        memory_total_bytes: 8192,
        temperature_c: 64.0,
        power_watts: Some(150.0),
        fan_percent: Some(45.0),
    }
}

fn smart() -> SmartHealth {
    SmartHealth {
        device: "/dev/nvme0".into(),
        model: "SN850".into(),
        health_passed: true,
        temperature_c: Some(39),
        power_on_hours: Some(1234),
        reallocated_sectors: Some(0),
        wear_level_percent: Some(7),
    }
}

fn snapshot(ts: u64) -> FullSystemSnapshot {
    FullSystemSnapshot {
        timestamp: ts,
        cpu: CpuStats::default(),
        ram: RamStats::default(),
        containers: vec![],
        storage: StorageStats::default(),
        network: NetworkStats::default(),
        system: SystemStatsDynamic::default(),
        gpus: vec![gpu()],
        smart: vec![smart()],
    }
}

#[tokio::test]
async fn raw_snapshot_round_trip_preserves_gpu_and_smart() {
    let dir = TempDir::new().unwrap();
    let repo = HistoryRepo::connect(dir.path().join("h.db").to_str().unwrap(), 3)
        .await
        .unwrap();
    repo.init().await.unwrap();

    repo.save_snapshots(&[snapshot(1_700_000_000_000)], &SystemInfo::default())
        .await
        .unwrap();

    let (_info, snaps) = repo.get_recent_snapshots(10).await.unwrap();
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].gpus, vec![gpu()]);
    assert_eq!(snaps[0].smart, vec![smart()]);
}

#[tokio::test]
async fn aggregated_snapshot_round_trip_preserves_gpu_and_smart() {
    let dir = TempDir::new().unwrap();
    let repo = HistoryRepo::connect(dir.path().join("h.db").to_str().unwrap(), 3)
        .await
        .unwrap();
    repo.init().await.unwrap();

    let agg = AggregatedSnapshot {
        created_at: 60_000,
        resolution_seconds: 60,
        cpu_load_avg: 10.0,
        cpu_load_min: 5.0,
        cpu_load_max: 15.0,
        memory_used_avg: 512,
        memory_used_min: 256,
        memory_used_max: 768,
        cpu: CpuStats::default(),
        ram: RamStats::default(),
        containers: vec![],
        storage: StorageStats::default(),
        network: NetworkStats::default(),
        system: SystemStatsDynamic::default(),
        gpus: vec![gpu()],
        smart: vec![smart()],
    };
    repo.save_aggregated_snapshot(&agg).await.unwrap();

    let rows = repo
        .get_aggregated_snapshots_by_time_range(0, 120_000, 60)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].gpus, vec![gpu()]);
    assert_eq!(rows[0].smart, vec![smart()]);
}
