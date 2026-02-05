use anyhow::Result;
use homeserver::*;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use tokio::sync::broadcast;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::FormatTime;

struct LocalTimer;

impl FormatTime for LocalTimer {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(
            w,
            "{}",
            chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%:z")
        )
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_timer(LocalTimer)
        .with_env_filter(filter)
        .init();

    let app_config = config::AppConfig::load()?;
    let (tx, _) =
        broadcast::channel::<models::FullSystemSnapshot>(app_config.publishing.broadcast_capacity);

    let sysinfo_repo = Arc::new(sysinfo_repo::SysinfoRepo::new());
    let system_info = Arc::new(
        sysinfo_repo
            .get_system_info()
            .await
            .map_err(|e| anyhow::anyhow!("system info: {}", e))?,
    );
    let docker_repo = Arc::new(docker_repo::DockerRepo::connect()?);
    let history_repo = Arc::new(
        history_repo::HistoryRepo::connect(
            &app_config.database.path,
            app_config.database.retention_days,
        )
        .await?,
    );
    history_repo.init().await?;

    let ws_system_connections = Arc::new(AtomicUsize::new(0));
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    let worker_handle = worker::spawn(
        worker::WorkerDeps {
            sysinfo_repo: sysinfo_repo.clone(),
            system_info: system_info.clone(),
            docker_repo: docker_repo.clone(),
            history_repo: history_repo.clone(),
            tx: tx.clone(),
            ws_system_connections: ws_system_connections.clone(),
            shutdown_rx,
        },
        worker::WorkerConfig {
            flush_rate: app_config.database.flush_rate,
            sample_interval_ms: app_config.monitoring.sample_interval_ms,
            stats_log_interval_secs: app_config.monitoring.stats_log_interval_secs,
        },
    );

    let app = routes::app(
        tx,
        sysinfo_repo,
        system_info,
        ws_system_connections,
        app_config.clone(),
    );
    let addr = format!("{}:{}", app_config.server.host, app_config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Listening on http://{}", addr);

    let in_container = std::path::Path::new("/.dockerenv").exists()
        || std::env::var("CONTAINER").as_deref() == Ok("1");

    if in_container {
        // In Docker: run server until error or SIGTERM (no signal handler; avoids immediate exit)
        axum::serve(listener, app).await?;
    } else {
        tokio::select! {
            result = axum::serve(listener, app) => {
                result?;
            }
            _ = async {
                #[cfg(unix)]
                {
                    let mut sigterm = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                        Ok(s) => s,
                        Err(_) => {
                            let _ = tokio::signal::ctrl_c().await;
                            return;
                        }
                    };
                    tokio::select! {
                        _ = tokio::signal::ctrl_c() => {}
                        _ = sigterm.recv() => {}
                    }
                }
                #[cfg(not(unix))]
                {
                    tokio::signal::ctrl_c().await
                }
            } => {
                tracing::info!("Received shutdown signal");
                let _ = shutdown_tx.send(());
                let _ = worker_handle.await;
            }
        }
    }

    Ok(())
}
