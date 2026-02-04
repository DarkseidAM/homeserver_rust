// WebSocket handlers and stream logic

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use bytes::Bytes;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use tokio::sync::broadcast;
use tokio::time::{Duration, timeout};

use super::AppState;
use crate::models::FullSystemSnapshot;
use crate::sysinfo_repo::SysinfoRepo;

pub(super) const WS_PING_INTERVAL: Duration = Duration::from_secs(30);
pub(super) const WS_SEND_TIMEOUT: Duration = Duration::from_secs(10);

/// Decrements ws_system connection count on drop (connect = +1, drop = -1).
struct WsSystemGuard(Arc<AtomicUsize>);

impl Drop for WsSystemGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

pub(super) async fn ws_cpu(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let repo = state.sysinfo_repo.clone();
    let interval_ms = state.config.publishing.cpu_stats_frequency_ms;
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = stream_cpu(socket, repo, interval_ms).await {
            tracing::info!("CPU stream error: {}", e);
        }
    })
}

async fn stream_cpu(
    mut socket: WebSocket,
    repo: Arc<SysinfoRepo>,
    interval_ms: u64,
) -> anyhow::Result<()> {
    tracing::info!("Client connected to CPU stream");
    let mut tick = tokio::time::interval(tokio::time::Duration::from_millis(interval_ms));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut ping_interval = tokio::time::interval(WS_PING_INTERVAL);
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let stats = repo.get_cpu_stats().await?;
                let json = serde_json::to_string(&stats)?;
                let r = timeout(WS_SEND_TIMEOUT, socket.send(Message::Text(json.into()))).await;
                if r.is_err() || r.unwrap_or(Ok(())).is_err() {
                    break;
                }
            }
            _ = ping_interval.tick() => {
                let r = timeout(WS_SEND_TIMEOUT, socket.send(Message::Ping(Bytes::new()))).await;
                if r.is_err() || r.unwrap_or(Ok(())).is_err() {
                    break;
                }
            }
        }
    }
    Ok(())
}

pub(super) async fn ws_ram(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let repo = state.sysinfo_repo.clone();
    let interval_ms = state.config.publishing.ram_stats_frequency_ms;
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = stream_ram(socket, repo, interval_ms).await {
            tracing::info!("RAM stream error: {}", e);
        }
    })
}

async fn stream_ram(
    mut socket: WebSocket,
    repo: Arc<SysinfoRepo>,
    interval_ms: u64,
) -> anyhow::Result<()> {
    tracing::info!("Client connected to RAM stream");
    let mut tick = tokio::time::interval(tokio::time::Duration::from_millis(interval_ms));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut ping_interval = tokio::time::interval(WS_PING_INTERVAL);
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let stats = repo.get_ram_stats().await?;
                let json = serde_json::to_string(&stats)?;
                let r = timeout(WS_SEND_TIMEOUT, socket.send(Message::Text(json.into()))).await;
                if r.is_err() || r.unwrap_or(Ok(())).is_err() {
                    break;
                }
            }
            _ = ping_interval.tick() => {
                let r = timeout(WS_SEND_TIMEOUT, socket.send(Message::Ping(Bytes::new()))).await;
                if r.is_err() || r.unwrap_or(Ok(())).is_err() {
                    break;
                }
            }
        }
    }
    Ok(())
}

pub(super) async fn ws_system(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let tx = state.stats_tx.clone();
    let conn_count = state.ws_system_connections.clone();
    let system_info = state.system_info.clone();
    ws.on_upgrade(move |socket| async move {
        let mut rx = tx.subscribe();
        if let Err(e) = stream_system(socket, &mut rx, conn_count, system_info).await {
            tracing::info!("System stream error: {}", e);
        }
    })
}

async fn stream_system(
    mut socket: WebSocket,
    rx: &mut broadcast::Receiver<FullSystemSnapshot>,
    conn_count: Arc<AtomicUsize>,
    system_info: Arc<crate::models::SystemInfo>,
) -> anyhow::Result<()> {
    conn_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let _guard = WsSystemGuard(conn_count);
    tracing::info!("Client connected to System stream");

    let welcome = serde_json::json!({ "type": "info", "systemInfo": system_info.as_ref() });
    let welcome_json = serde_json::to_string(&welcome)?;
    let r = timeout(
        WS_SEND_TIMEOUT,
        socket.send(Message::Text(welcome_json.into())),
    )
    .await;
    if r.is_err() || r.unwrap_or(Ok(())).is_err() {
        return Ok(());
    }

    let mut ping_interval = tokio::time::interval(WS_PING_INTERVAL);
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(snapshot) => {
                        let json = serde_json::to_string(&snapshot)?;
                        let r = timeout(WS_SEND_TIMEOUT, socket.send(Message::Text(json.into()))).await;
                        if r.is_err() || r.unwrap_or(Ok(())).is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("WebSocket /ws/system client lagged, skipped {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            _ = ping_interval.tick() => {
                let r = timeout(WS_SEND_TIMEOUT, socket.send(Message::Ping(Bytes::new()))).await;
                if r.is_err() || r.unwrap_or(Ok(())).is_err() {
                    break;
                }
            }
        }
    }
    Ok(())
}
