// Worker integration test: spawn, tick, shutdown, assert history flushed

use homeserver::docker_repo::DockerRepo;
use homeserver::history_repo::HistoryRepo;
use homeserver::sysinfo_repo::SysinfoRepo;
use homeserver::worker::{WorkerConfig, WorkerDeps, spawn};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
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
    let history_repo = Arc::new(HistoryRepo::connect(path_str).await.unwrap());
    history_repo.init().await.unwrap();

    let (tx, _rx) = broadcast::channel(10);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let ws_system_connections = Arc::new(AtomicUsize::new(0));

    let deps = WorkerDeps {
        sysinfo_repo,
        system_info,
        docker_repo,
        history_repo: history_repo.clone(),
        tx,
        ws_system_connections,
        shutdown_rx,
    };
    let config = WorkerConfig {
        flush_rate: 2,
        sample_interval_ms: 25,
        stats_log_interval_secs: 3600,
    };

    let handle = spawn(deps, config);
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
    let _ = shutdown_tx.send(());
    handle.await.unwrap();

    let (_info, recent) = history_repo.get_recent_snapshots(100).await.unwrap();
    assert!(
        !recent.is_empty(),
        "worker should have flushed at least one snapshot (periodic or on shutdown)"
    );
}
