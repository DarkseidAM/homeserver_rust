// Integration tests: HTTP and WebSocket endpoints

use axum_test::TestServer;
use homeserver::config::AppConfig;
use homeserver::models::{CpuStats, FullSystemSnapshot, RamStats};
use homeserver::routes;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use tokio::sync::broadcast;

const TEST_CONFIG: &str = r#"
[server]
port = 8081
host = "0.0.0.0"

[database]
path = "data/test.db"
max_pool_size = 2
flush_rate = 5

[publishing]
cpu_stats_frequency_ms = 1000
ram_stats_frequency_ms = 1000
broadcast_capacity = 10

[monitoring]
sample_interval_ms = 1000
stats_log_interval_secs = 60
"#;

fn test_app_config() -> AppConfig {
    AppConfig::load_from_str(TEST_CONFIG).unwrap()
}

fn test_app() -> (
    axum::Router,
    broadcast::Sender<homeserver::models::FullSystemSnapshot>,
) {
    let config = test_app_config();
    let (tx, _) = broadcast::channel(config.publishing.broadcast_capacity);
    let app = routes::app(
        tx.clone(),
        Arc::new(homeserver::sysinfo_repo::SysinfoRepo::new()),
        Arc::new(AtomicUsize::new(0)),
        config,
    );
    (app, tx)
}

/// Build TestServer with http_transport (required for WebSocket tests).
fn test_server_with_http() -> (
    TestServer,
    broadcast::Sender<homeserver::models::FullSystemSnapshot>,
) {
    let (app, tx) = test_app();
    let server = TestServer::builder().http_transport().build(app).unwrap();
    (server, tx)
}

#[tokio::test]
async fn test_root_endpoint() {
    let (app, _) = test_app();
    let server = TestServer::new(app).unwrap();
    let response = server.get("/").await;
    response.assert_status_ok();
    response.assert_text("Ktor: Hello from Rust homeserver!");
}

#[tokio::test]
async fn test_version_endpoint() {
    let (app, _) = test_app();
    let server = TestServer::new(app).unwrap();
    let response = server.get("/version").await;
    response.assert_status_ok();
    let json: serde_json::Value = response.json();
    assert_eq!(
        json.get("name").and_then(|v| v.as_str()),
        Some("homeserver")
    );
    assert!(json.get("version").and_then(|v| v.as_str()).is_some());
}

// --- WebSocket message tests (require http_transport + ws feature) ---
// Receive until we get valid JSON (server may send Ping first).

async fn receive_first_json_text<T: serde::de::DeserializeOwned>(
    ws: &mut axum_test::TestWebSocket,
) -> T {
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(3);
    loop {
        let text = ws.receive_text().await;
        if let Ok(v) = serde_json::from_str::<T>(&text) {
            return v;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for JSON"
        );
    }
}

#[tokio::test]
async fn test_ws_cpu_receives_json() {
    let (server, _) = test_server_with_http();
    let mut ws = server.get_websocket("/ws/cpu").await.into_websocket().await;
    let _cpu: CpuStats = receive_first_json_text(&mut ws).await;
}

#[tokio::test]
async fn test_ws_ram_receives_json() {
    let (server, _) = test_server_with_http();
    let mut ws = server.get_websocket("/ws/ram").await.into_websocket().await;
    let _ram: RamStats = receive_first_json_text(&mut ws).await;
}

#[tokio::test]
async fn test_ws_system_receives_broadcast_snapshot() {
    let (server, tx) = test_server_with_http();
    let snapshot = FullSystemSnapshot {
        timestamp: 42,
        cpu: CpuStats {
            model: "test".into(),
            physical_cores: 1,
            logical_cores: 2,
            usage_percent: 0.0,
            temperature: 0.0,
        },
        ram: RamStats {
            total: 100,
            used: 50,
            available: 50,
            usage_percent: 50.0,
        },
        containers: vec![],
        storage: homeserver::models::StorageStats {
            partitions: vec![],
            disks: vec![],
        },
        network: homeserver::models::NetworkStats { interfaces: vec![] },
        system: homeserver::models::SystemStats {
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
    };
    let mut ws = server
        .get_websocket("/ws/system")
        .await
        .into_websocket()
        .await;
    let tx_clone = tx.clone();
    let snapshot_clone = snapshot.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let _ = tx_clone.send(snapshot_clone);
    });
    let received: FullSystemSnapshot = receive_first_json_text(&mut ws).await;
    assert_eq!(received.timestamp, 42);
    assert_eq!(received.ram.used, 50);
}
