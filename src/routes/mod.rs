// HTTP + WebSocket routes

mod http;
mod ws;

use axum::{Router, routing::get};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};

use crate::config::AppConfig;
use crate::models::{FullSystemSnapshot, SystemInfo};
use crate::sysinfo_repo::SysinfoRepo;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) stats_tx: broadcast::Sender<FullSystemSnapshot>,
    pub(crate) sysinfo_repo: Arc<SysinfoRepo>,
    pub(crate) system_info: Arc<SystemInfo>,
    pub(crate) ws_system_connections: Arc<AtomicUsize>,
    pub(crate) config: AppConfig,
}

pub fn app(
    stats_tx: broadcast::Sender<FullSystemSnapshot>,
    sysinfo_repo: Arc<SysinfoRepo>,
    system_info: Arc<SystemInfo>,
    ws_system_connections: Arc<AtomicUsize>,
    config: AppConfig,
) -> Router {
    let state = AppState {
        stats_tx,
        sysinfo_repo,
        system_info,
        ws_system_connections,
        config,
    };
    Router::new()
        .route("/", get(|| async { "Ktor: Hello from Rust homeserver!" })) // GET /
        .route("/version", get(http::version_handler)) // GET /version
        .route("/api/info", get(http::api_info_handler)) // GET /api/info
        .route("/ws/cpu", get(ws::ws_cpu)) // WS /ws/cpu
        .route("/ws/ram", get(ws::ws_ram)) // WS /ws/ram
        .route("/ws/system", get(ws::ws_system)) // WS /ws/system
        .layer(CorsLayer::new().allow_origin(Any))
        .with_state(state)
}
