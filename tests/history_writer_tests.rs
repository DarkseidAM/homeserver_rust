// Verifies the history writer's persist_gpu / persist_smart gating: when disabled, GPU/SMART
// are stripped before persisting (live WS is unaffected); when enabled, they are written.

use homeserver::history_repo::HistoryRepo;
use homeserver::models::*;
use homeserver::worker::{HistoryWriterConfig, spawn_history_writer};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use tempfile::TempDir;

fn snapshot_with_gpu_smart(ts: u64) -> FullSystemSnapshot {
    FullSystemSnapshot {
        timestamp: ts,
        cpu: CpuStats::default(),
        ram: RamStats::default(),
        containers: vec![],
        storage: StorageStats::default(),
        network: NetworkStats::default(),
        system: SystemStatsDynamic::default(),
        gpus: vec![GpuStats {
            vendor: "amd".into(),
            ..Default::default()
        }],
        smart: vec![SmartHealth {
            device: "/dev/sda".into(),
            ..Default::default()
        }],
    }
}

/// Run the writer once with the given persistence flags and return the persisted snapshot.
async fn persist_and_read(persist_gpu: bool, persist_smart: bool) -> FullSystemSnapshot {
    let dir = TempDir::new().unwrap();
    let repo = Arc::new(
        HistoryRepo::connect(dir.path().join("h.db").to_str().unwrap(), 3)
            .await
            .unwrap(),
    );
    repo.init().await.unwrap();

    let (tx, rx) = tokio::sync::mpsc::channel(8);
    let handle = spawn_history_writer(
        rx,
        repo.clone(),
        Arc::new(SystemInfo::default()),
        HistoryWriterConfig {
            flush_rate: 1,
            flush_interval_secs: 3600,
            persist_gpu,
            persist_smart,
        },
        Arc::new(AtomicU64::new(0)),
    );

    tx.send(snapshot_with_gpu_smart(1_700_000_000_000))
        .await
        .unwrap();
    drop(tx); // closing the channel triggers a final flush, then the task exits
    handle.await.unwrap();

    let (_info, snaps) = repo.get_recent_snapshots(10).await.unwrap();
    assert_eq!(snaps.len(), 1, "snapshot was persisted");
    snaps.into_iter().next().unwrap()
}

#[tokio::test]
async fn persist_flags_off_strips_gpu_and_smart() {
    let snap = persist_and_read(false, false).await;
    assert!(snap.gpus.is_empty(), "GPU stripped when persist_gpu=false");
    assert!(
        snap.smart.is_empty(),
        "SMART stripped when persist_smart=false"
    );
}

#[tokio::test]
async fn persist_flags_on_keeps_gpu_and_smart() {
    let snap = persist_and_read(true, true).await;
    assert_eq!(snap.gpus.len(), 1, "GPU persisted when persist_gpu=true");
    assert_eq!(
        snap.smart.len(),
        1,
        "SMART persisted when persist_smart=true"
    );
}
