// WebSocket handlers and stream logic.
//
// Uses `yawc` for the WebSocket transport so connections negotiate permessage-deflate
// (RFC 7692) compression. The socket is split into a sink + stream: the sink sends
// stats/pings, while the stream is polled so client Close frames terminate the loop
// promptly (and pongs are drained).

use axum::{
    extract::State,
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use tokio::sync::broadcast;
use tokio::time::{Duration, timeout};
use yawc::frame::{Frame, OpCode};
use yawc::{IncomingUpgrade, Options};

use super::AppState;
use crate::models::{FullSystemSnapshot, SystemInfo};

pub(super) const WS_PING_INTERVAL: Duration = Duration::from_secs(30);
pub(super) const WS_SEND_TIMEOUT: Duration = Duration::from_secs(10);

/// WebSocket options: balanced permessage-deflate compression, always on (clients negotiate).
fn ws_options() -> Options {
    Options::default().with_balanced_compression()
}

/// Decrements ws_system connection count on drop (connect = +1, drop = -1).
struct WsSystemGuard(Arc<AtomicUsize>);

impl Drop for WsSystemGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Send a frame under the standard timeout. Returns false if it timed out or errored.
async fn send_frame<S>(sink: &mut S, frame: Frame) -> bool
where
    S: futures_util::Sink<Frame> + Unpin,
{
    matches!(timeout(WS_SEND_TIMEOUT, sink.send(frame)).await, Ok(Ok(())))
}

/// True if an inbound stream item means the connection should close (peer closed / stream ended).
fn is_close(incoming: &Option<Frame>) -> bool {
    match incoming {
        None => true,
        Some(frame) => frame.opcode() == OpCode::Close,
    }
}

pub(super) async fn ws_cpu(ws: IncomingUpgrade, State(state): State<AppState>) -> Response {
    let repo = state.sysinfo_repo.clone();
    let interval_ms = state.config.publishing.cpu_stats_frequency_ms;
    upgrade(ws, "cpu", move |socket| async move {
        let (sink, stream) = socket.split();
        pump_periodic(sink, stream, interval_ms, move || {
            let repo = repo.clone();
            async move { repo.get_cpu_stats().await }
        })
        .await;
    })
}

pub(super) async fn ws_ram(ws: IncomingUpgrade, State(state): State<AppState>) -> Response {
    let repo = state.sysinfo_repo.clone();
    let interval_ms = state.config.publishing.ram_stats_frequency_ms;
    upgrade(ws, "ram", move |socket| async move {
        let (sink, stream) = socket.split();
        pump_periodic(sink, stream, interval_ms, move || {
            let repo = repo.clone();
            async move { repo.get_ram_stats().await }
        })
        .await;
    })
}

pub(super) async fn ws_system(ws: IncomingUpgrade, State(state): State<AppState>) -> Response {
    let tx = state.stats_tx.clone();
    let conn_count = state.ws_system_connections.clone();
    let system_info = state.system_info.clone();
    upgrade(ws, "system", move |socket| async move {
        let mut rx = tx.subscribe();
        stream_system(socket, &mut rx, conn_count, system_info).await;
    })
}

/// Completes the upgrade and spawns `run` with the established WebSocket. Returns the HTTP
/// upgrade response (or 400 if the handshake request is malformed). `_repo` etc. are captured
/// by the `run` closure.
fn upgrade<F, Fut>(ws: IncomingUpgrade, stream: &'static str, run: F) -> Response
where
    F: FnOnce(yawc::HttpWebSocket) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let (response, fut) = match ws.upgrade(ws_options()) {
        Ok(v) => v,
        Err(e) => {
            tracing::info!(error = %e, stream, "WebSocket upgrade rejected");
            return axum::http::StatusCode::BAD_REQUEST.into_response();
        }
    };
    tokio::spawn(async move {
        match fut.await {
            Ok(socket) => {
                tracing::info!(stream, "WebSocket client connected");
                run(socket).await;
            }
            Err(e) => tracing::info!(error = %e, stream, "WebSocket handshake failed"),
        }
    });
    response.into_response()
}

/// Periodically fetch a serializable stat and push it as a text frame; ping on `WS_PING_INTERVAL`;
/// stop when the peer closes, a send times out, or fetching fails.
async fn pump_periodic<Si, St, F, Fut, T>(mut sink: Si, mut stream: St, interval_ms: u64, fetch: F)
where
    Si: futures_util::Sink<Frame> + Unpin,
    St: futures_util::Stream<Item = Frame> + Unpin,
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
    T: serde::Serialize,
{
    let mut tick = tokio::time::interval(Duration::from_millis(interval_ms));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut ping = tokio::time::interval(WS_PING_INTERVAL);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let stats = match fetch().await {
                    Ok(s) => s,
                    Err(e) => { tracing::info!(error = %e, "WebSocket stat fetch failed"); break; }
                };
                let Ok(json) = serde_json::to_string(&stats) else { break };
                if !send_frame(&mut sink, Frame::text(json)).await {
                    break;
                }
            }
            _ = ping.tick() => {
                if !send_frame(&mut sink, Frame::ping(Bytes::new())).await {
                    break;
                }
            }
            incoming = stream.next() => {
                if is_close(&incoming) {
                    break;
                }
            }
        }
    }
}

/// `/ws/system`: send a welcome with static system info, then re-broadcast every snapshot.
async fn stream_system<Ws>(
    socket: Ws,
    rx: &mut broadcast::Receiver<FullSystemSnapshot>,
    conn_count: Arc<AtomicUsize>,
    system_info: Arc<SystemInfo>,
) where
    Ws: futures_util::Sink<Frame> + futures_util::Stream<Item = Frame> + Unpin,
{
    let current = conn_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    let _guard = WsSystemGuard(conn_count.clone());
    tracing::info!(
        connections = current,
        stream = "system",
        "System stream subscribed"
    );

    let (mut sink, mut stream) = socket.split();

    let welcome = serde_json::json!({ "type": "info", "systemInfo": system_info.as_ref() });
    let Ok(welcome_json) = serde_json::to_string(&welcome) else {
        return;
    };
    if !send_frame(&mut sink, Frame::text(welcome_json)).await {
        return;
    }

    let mut ping = tokio::time::interval(WS_PING_INTERVAL);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(snapshot) => {
                        let Ok(json) = serde_json::to_string(&snapshot) else { break };
                        if !send_frame(&mut sink, Frame::text(json)).await {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(messages_skipped = n, stream = "system", "WebSocket client lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            _ = ping.tick() => {
                if !send_frame(&mut sink, Frame::ping(Bytes::new())).await {
                    break;
                }
            }
            incoming = stream.next() => {
                if is_close(&incoming) {
                    break;
                }
            }
        }
    }
}
