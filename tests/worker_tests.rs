// Worker integration test: spawn collector + writer, tick, shutdown, assert history flushed

use homeserver::docker_repo::DockerRepo;
use homeserver::history_repo::HistoryRepo;
use homeserver::sysinfo_repo::SysinfoRepo;
use homeserver::worker::{
    HistoryWriterConfig, WorkerConfig, WorkerDeps, spawn, spawn_history_writer,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize};
use tokio::sync::broadcast;

#[tokio::test]
async fn worker_spawn_ticks_and_shutdown_flushes_history() {
    let docker_repo = match DockerRepo::connect() {
        Ok(r) => Arc::new(r),
        Err(_) => return, // Skip when Docker is not available
    };

    let sysinfo_repo = Arc::new(SysinfoRepo::new());
    let system_info = Arc::new(
        sysinfo_repo
            .get_system_info()
            .await
            .expect("get_system_info"),
    );

    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("history.db");
    let path_str = db_path.to_str().unwrap();
    let history_repo = Arc::new(HistoryRepo::connect(path_str, 3).await.unwrap());
    history_repo.init().await.unwrap();

    let (tx, _rx) = broadcast::channel(10);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let ws_system_connections = Arc::new(AtomicUsize::new(0));
    let snapshots_saved_total = Arc::new(AtomicU64::new(0));

    let writer_capacity = homeserver::worker::writer_channel_capacity(2);
    let (write_tx, write_rx) = tokio::sync::mpsc::channel(writer_capacity);
    let writer_handle = spawn_history_writer(
        write_rx,
        history_repo.clone(),
        system_info.clone(),
        HistoryWriterConfig {
            flush_rate: 2,
            flush_interval_secs: 60,
        },
        snapshots_saved_total.clone(),
    );

    let deps = WorkerDeps {
        sysinfo_repo,
        system_info,
        docker_repo,
        history_repo: history_repo.clone(),
        tx,
        write_tx,
        ws_system_connections,
        snapshots_saved_total,
        shutdown_rx,
    };
    let config = WorkerConfig {
        sample_interval_ms: 25,
        stats_log_interval_secs: 3600,
        prune_interval_secs: 3600,
    };

    let worker_handle = spawn(deps, config);
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
    let _ = shutdown_tx.send(());
    worker_handle.await.unwrap();
    writer_handle.await.unwrap();

    let (_info, recent) = history_repo.get_recent_snapshots(100).await.unwrap();
    assert!(
        !recent.is_empty(),
        "worker should have flushed at least one snapshot (via writer on shutdown)"
    );
}
