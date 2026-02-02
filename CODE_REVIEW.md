# In-Depth Code Review: Rust Homeserver

**Date**: January 31, 2026  
**Reviewer**: Code Analysis  
**Status**: Initial Review

---

## Overview

This is a well-structured Rust port of a Kotlin system monitoring server. The code is clean, functional, and follows good Rust practices. However, there are several areas where it can be improved for better performance, maintainability, reliability, and production-readiness.

---

## 1. Architecture & Design

### Strengths
- Clean separation of concerns (repos, routes, worker, models)
- Good use of Arc for shared state
- Appropriate choice of libraries (axum, tokio, sysinfo, bollard)

### Issues & Suggestions

#### 1.1 Blocking operations in async context ✅ FIXED

**Location**: `src/sysinfo_repo.rs:20-24`

**Problem**: `std::thread::sleep` blocks the entire tokio thread, which can starve other tasks.

**Status**: ✅ **COMPLETED** - All sysinfo methods now use `tokio::task::spawn_blocking` to prevent blocking the async runtime.

**Changes Made**:
- Converted `get_cpu_stats()` to async with `spawn_blocking`
- Converted `get_ram_stats()` to async with `spawn_blocking`
- Converted `get_storage_stats()` to async with `spawn_blocking`
- Converted `get_network_stats()` to async with `spawn_blocking`
- Converted `get_system_stats()` to async with `spawn_blocking`
- Updated all callers in `worker.rs` and `routes.rs` to await these calls

**Impact**: High - Prevents blocking the tokio runtime and improves performance under load

---

#### 1.2 std::sync::Mutex in async code ✅ NOT APPLICABLE

**Location**: `src/sysinfo_repo.rs:8`

**Problem**: The use of `std::sync::Mutex` can cause issues in async contexts.

**Status**: ✅ **NOT APPLICABLE** – Lock is only acquired inside `tokio::task::spawn_blocking()`. The mutex is never held across an `.await`, so `std::sync::Mutex` is the correct choice here. Using `tokio::sync::Mutex` would require either holding the lock across a blocking call or calling `block_on(mutex.lock())` inside the blocking task (anti-pattern). No change needed.

---

## 2. Error Handling

### Issues

#### 2.1 Unwrap usage throughout codebase ✅ FIXED

**Location**: Multiple files (`src/sysinfo_repo.rs`, `src/history_repo.rs`, `src/worker.rs`)

**Problem**: `.unwrap()` panics on error, crashing the entire server if a lock is poisoned.

**Status**: ✅ **COMPLETED**

**Changes Made**:
- **history_repo.rs**: All 3 `conn.lock().unwrap()` replaced with `.lock().map_err(|e| anyhow::anyhow!("database lock poisoned: {}", e))?`
- **sysinfo_repo.rs**: All 5 methods now return `anyhow::Result<T>`. Lock uses `.map_err(...)?`, spawn_blocking join uses `.await.map_err(...)?`. Closure returns `Ok(...)`.
- **worker.rs**: `duration_since(UNIX_EPOCH).unwrap()` replaced with `.map(...).unwrap_or_else(|e| { tracing::warn!(...); 0 })`. All sysinfo calls use `match ... { Ok(c) => c, Err(e) => { tracing::warn!(...); continue; } }` to skip the cycle on error.
- **routes.rs**: `stream_cpu` and `stream_ram` handle `get_cpu_stats()` / `get_ram_stats()` Result with match; on error log and break (close stream).

---

#### 2.2 Silent errors in worker ✅ FIXED

**Location**: `src/worker.rs`

**Problem**: Broadcast send errors are silently ignored. If all receivers are dropped, this indicates a problem.

**Status**: ✅ **COMPLETED** – Worker now logs when `tx.send(...)` fails (no active WebSocket clients).

---

#### 2.3 Inconsistent error handling patterns ✅ FIXED

**Problem**: Some functions return `anyhow::Result`, others return unit and log errors internally.

**Status**: ✅ **COMPLETED** – Stream handlers no longer log errors internally; they propagate with `?`. Only the ws_* handlers (callers) log on `stream_*` failure. Worker already follows “return Result, caller logs” for save_snapshots/prune_old_data.

---

## 3. Performance Issues

#### 3.1 Unnecessary cloning in worker ✅ NO ACTION NEEDED

**Location**: `src/worker.rs`

**What’s going on**: We need the same snapshot in two places: (1) sent to the broadcast channel (WebSocket clients), (2) pushed into `snapshot_buffer` (for DB flush). In Rust, passing a value to a function usually *moves* it, so we’d lose it for the second use.

**What the code does**:
- `tx.send(snapshot.clone())` — sends a *copy* of `snapshot` into the channel; the original `snapshot` is still ours.
- `snapshot_buffer.push(snapshot)` — moves the *original* into the buffer (no extra clone).

So we do **one** clone (for the channel) and **one** move (into the buffer). There is no unnecessary second clone; the “last use” (push) already doesn’t clone.

**Status**: ✅ **NO ACTION NEEDED** – Current code is correct and as efficient as it can be here.

---

#### 3.2 Refreshing system info on every call ✅ FIXED

**Location**: `src/sysinfo_repo.rs`

**Problem**: Creating new `Disks` and `Networks` instances on every call was expensive.

**Status**: ✅ **COMPLETED** – SysinfoRepo now holds persistent `Arc<Mutex<Disks>>` and `Arc<Mutex<Networks>>`. In `new()` we create them once with `new_with_refreshed_list()`. In `get_storage_stats` / `get_network_stats` we lock, call `refresh(false)` / `refresh(true)`, then iterate over `list()`. Reduces allocation and system calls.

---

#### 3.3 SQL statement preparation inefficiency ✅ NO ACTION NEEDED

**Location**: `src/history_repo.rs`

**Problem**: Statement is prepared inside a transaction, which might not benefit from rusqlite's cache optimally.

**Status**: ✅ **NO ACTION NEEDED** – In rusqlite, `Transaction` derefs to `Connection`, so `tx.prepare_cached()` uses the **connection’s** statement cache. The same prepared statement is reused on later `save_snapshots` calls. We have to prepare inside the transaction because of Rust’s borrow rules (we can’t hold both a cached statement and a transaction that both use the same connection). A short comment was added in code. Impact was low; no change required.

---

#### 3.4 JSON serialization in hot path ✅ FIXED

**Location**: `src/history_repo.rs`

**Problem**: JSON serialization happens for every snapshot in the batch.

**Status**: ✅ **FIXED** – Schema versioning was added: new DBs use schema v2 with BLOB columns and **bincode** for faster, smaller serialization; existing DBs stay on schema v1 (TEXT/JSON). `schema_version` table and `get_schema_version()` determine which path to use in `save_snapshots`. No readers of history exist in this codebase, so binary storage is safe.

**Impact**: Low - Optimization for new installs; legacy DBs unchanged

---

## 4. Concurrency & Safety

#### 4.1 Race condition in worker ticks ✅ FIXED

**Location**: `src/worker.rs`

**Problem**: If `flush_rate` doesn't evenly divide `PRUNE_INTERVAL_TICKS`, pruning may never occur correctly.

**Status**: ✅ **FIXED** – Separate tick counters `flush_ticks` and `prune_ticks` are used; flush and prune run independently.

```rust
if ticks % flush_rate == 0 && !snapshot_buffer.is_empty() {
    if let Err(e) = history_repo.save_snapshots(&snapshot_buffer) {
        tracing::warn!("Failed to save snapshots: {}", e);
    }
    snapshot_buffer.clear();
    if ticks > 0 && ticks % PRUNE_INTERVAL_TICKS == 0 {
        if let Err(e) = history_repo.prune_old_data() {
            tracing::warn!("Failed to prune old data: {}", e);
        }
    }
}
```

**Solution**: Use separate tick counters:

```rust
let mut flush_ticks = 0;
let mut prune_ticks = 0;

loop {
    tick.tick().await;
    
    // ... collect snapshot ...
    
    flush_ticks += 1;
    if flush_ticks >= flush_rate && !snapshot_buffer.is_empty() {
        history_repo.save_snapshots(&snapshot_buffer)?;
        snapshot_buffer.clear();
        flush_ticks = 0;
    }
    
    prune_ticks += 1;
    if prune_ticks >= PRUNE_INTERVAL_TICKS {
        history_repo.prune_old_data()?;
        prune_ticks = 0;
    }
}
```

**Impact**: Medium - Ensures pruning works correctly

---

#### 4.2 No graceful shutdown ✅ FIXED

**Location**: `src/main.rs`, `src/worker.rs`

**Problem**: Worker was aborted abruptly, which could lose buffered snapshots.

**Status**: ✅ **FIXED** – Shutdown oneshot channel added; worker selects on tick vs shutdown and flushes `snapshot_buffer` before exiting. Main uses `tokio::select!` between `axum::serve` and shutdown signal(s). On Unix, both SIGINT (Ctrl+C) and **SIGTERM** (e.g. `docker stop`) trigger graceful shutdown so it works when run in a container.

```rust
worker_handle.abort();
```

**Solution**: Use a shutdown channel to gracefully stop the worker and flush remaining data:

```rust
// In main.rs
let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

let worker_handle = worker::spawn(
    sysinfo_repo.clone(),
    docker_repo.clone(),
    history_repo.clone(),
    tx.clone(),
    app_config.database.flush_rate,
    shutdown_rx,
);

// Signal handling
tokio::select! {
    result = axum::serve(listener, app) => {
        result?;
    }
    _ = tokio::signal::ctrl_c() => {
        tracing::info!("Received shutdown signal");
        let _ = shutdown_tx.send(());
        // Wait for worker to finish
        let _ = worker_handle.await;
    }
}
```

**Impact**: High - Prevents data loss on shutdown

---

#### 4.3 Integer overflow in tick counter

**Location**: `src/worker.rs:67`

**Problem**: `ticks` will eventually overflow after ~584 million years at 1s intervals, but still worth noting.

```rust
ticks += 1;
```

**Solution**: Use wrapping arithmetic or u128, or reset counter:

```rust
ticks = ticks.wrapping_add(1);
```

**Impact**: Very Low - Theoretical issue

---

## 5. Resource Management

#### 5.1 No connection pooling for SQLite ✅ FIXED (sqlx + pool)

**Location**: `src/history_repo.rs`

**Problem**: Single connection wrapped in Mutex could become a bottleneck.

**Status**: ✅ **FIXED** – Migrated from rusqlite to **sqlx 0.8**: `SqlitePool` with `SqlitePoolOptions`, async `connect()` and `init()`, WAL + `busy_timeout` + `synchronous=NORMAL` via `SqliteConnectOptions`. Single schema only (BLOB/wincode); v1/v2 logic removed. Future migrations: see doc comment in `history_repo.rs` (schema_version table kept for optional versioned migrations).

```rust
let conn = Connection::open(path)?;
Ok(Self {
    conn: Mutex::new(conn),
})
```

**Solution**: Either:
- Accept the single-writer limitation and document it (SQLite's Write-Ahead Logging helps)
- Consider using `r2d2` with `r2d2_sqlite` for connection pooling
- Use `rusqlite` async support if available

**Impact**: Low - Current approach is reasonable for this use case

---

#### 5.2 Broadcast channel capacity ✅ FIXED

**Location**: `src/main.rs`, `config.toml`, `src/config.rs`

**Problem**: Capacity of 16 was quite small; slow consumers would lag and miss messages.

**Status**: ✅ **FIXED** – Capacity is configurable via `publishing.broadcast_capacity` in config.toml (default 60). Main creates the channel with `app_config.publishing.broadcast_capacity`. Lagged error handling in `/ws/system` consumers not yet added (optional follow-up).

**Impact**: Medium - Affects reliability under load

---

#### 5.3 Docker stats streams not cleaned up on error ✅ FIXED

**Location**: `src/docker_repo.rs`

**Problem**: If the stream errored (not just returned None), the handle stayed in `active_streams` until the container stopped, causing a resource leak.

**Status**: ✅ **FIXED** – The spawned task now receives a clone of `active_streams` and, when the loop exits (stream end or error), calls `active_streams.write().await.remove(&id)` so the handle is removed. Stream errors are matched explicitly (`Err(e)` → log and break). The next `list_running_and_refresh_stats` can then spawn a new task for that container if it is still running.

**Impact**: Medium - Prevents resource leaks

---

#### 5.4 No limit on number of Docker containers

**Problem**: If someone runs hundreds of containers, we spawn hundreds of stats streams.

**Solution**: Add configuration for max monitored containers or implement prioritization.

**Impact**: Low - Edge case for most deployments

---

## 6. Configuration & Observability

#### 6.1 Config validation missing ✅ FIXED

**Location**: `src/config.rs`

**Problem**: No validation of values (e.g., port range, interval > 0).

**Status**: ✅ **FIXED** – `AppConfig::load()` now calls `config.validate()` after parsing. `validate()` checks: `server.port` > 0; `database.path` non-empty; `database.max_pool_size` > 0; `database.flush_rate` > 0; `publishing.cpu_stats_frequency_ms` > 0; `publishing.ram_stats_frequency_ms` > 0; `publishing.broadcast_capacity` > 0; `monitoring.sample_interval_ms` > 0. Uses `anyhow::ensure!` with clear messages.

```rust
impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let path = std::env::var("CONFIG_FILE").unwrap_or_else(|_| "config.toml".into());
        let s = std::fs::read_to_string(&path)?;
        let config: AppConfig = toml::from_str(&s)?;
        Ok(config)
    }
}
```

**Solution**: Add validation:

```rust
pub fn load() -> anyhow::Result<Self> {
    let path = std::env::var("CONFIG_FILE").unwrap_or_else(|_| "config.toml".into());
    let s = std::fs::read_to_string(&path)?;
    let config: AppConfig = toml::from_str(&s)?;
    config.validate()?;
    Ok(config)
}

impl AppConfig {
    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(self.server.port > 0 && self.server.port < 65536, 
            "Port must be between 1 and 65535");
        anyhow::ensure!(self.database.flush_rate > 0, 
            "Flush rate must be > 0");
        anyhow::ensure!(self.publishing.cpu_stats_frequency_ms > 0, 
            "CPU frequency must be > 0");
        anyhow::ensure!(self.publishing.ram_stats_frequency_ms > 0, 
            "RAM frequency must be > 0");
        anyhow::ensure!(self.monitoring.sample_interval_ms > 0, 
            "Sample interval must be > 0");
        Ok(())
    }
}
```

**Impact**: Medium - Prevents misconfigurations

---

#### 6.2 No metrics/monitoring ✅ ADDRESSED (periodic INFO logs)

**Problem**: No visibility into application health and performance.

**Status**: ✅ **ADDRESSED** – Instead of Prometheus, app stats are logged at a configurable interval: `monitoring.stats_log_interval_secs` (default 60). The worker logs one INFO line every N seconds with `ws_system_clients`, `snapshots_saved_total`, `snapshots_pruned_total`. `/ws/system` connection count is tracked via `Arc<AtomicUsize>` (increment on connect, decrement on disconnect via `WsSystemGuard`). No new dependencies; logs can be scraped or tailed as needed.

**Impact**: Medium - Lightweight app observability without Prometheus

---

#### 6.3 Log levels not optimal

**Location**: `src/routes.rs:61, 93, 121`

**Problem**: Client connections logged at INFO level will be noisy in production.

```rust
tracing::info!("Client connected to CPU stream");
tracing::info!("Client connected to RAM stream");
tracing::info!("Client connected to System stream");
```

**Solution**: Use DEBUG level for connection events:

```rust
tracing::debug!("Client connected to CPU stream");
```

Use INFO for lifecycle events (server start/stop, worker start, etc.).

**Impact**: Low - Log hygiene

---

#### 6.4 Hardcoded constant doesn't match config ✅ FIXED

**Location**: `src/worker.rs`, `src/main.rs`

**Problem**: `SAMPLE_INTERVAL_MS` was hardcoded but config has `monitoring.sample_interval_ms`.

**Status**: ✅ **FIXED** – Removed the constant; `worker::spawn` now takes `sample_interval_ms: u64` and main passes `app_config.monitoring.sample_interval_ms`. Worker tick interval is driven by config.

```rust
const SAMPLE_INTERVAL_MS: u64 = 1000;
```

**Solution**: Remove constant and use config value:

```rust
pub fn spawn(
    // ... existing params ...
    sample_interval_ms: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_millis(sample_interval_ms));
        // ...
    })
}
```

**Impact**: Medium - Config should be respected

---

## 7. Testing

#### 7.1 No actual tests ✅ FIXED

**Location**: `tests/`

**Problem**: All tests were TODO stubs, providing zero coverage.

**Status**: ✅ **FIXED** – Implemented: (1) **config_tests**: `load_from_str` with valid TOML, assert sections; validation rejects port 0 and empty db path. (2) **models_tests**: CpuStats, ContainerStats, FullSystemSnapshot JSON serialize/roundtrip and camelCase. (3) **integration_tests**: `TestServer` with routes::app, GET `/` asserts status and body. Added `AppConfig::load_from_str` for testing. No WebSocket or worker E2E tests yet.

**Impact**: Critical - Basic coverage for config, models, root endpoint

---

#### 7.2 Missing dev dependencies ✅ FIXED

**Problem**: No test utilities in Cargo.toml.

**Status**: ✅ **FIXED** – Added `[dev-dependencies]`: `tokio` (rt, macros), `tempfile`, `axum-test` (18).

**Impact**: Medium - Enables test development

---

## 8. WebSocket Improvements

#### 8.1 No ping/pong for connection health

**Location**: `src/routes.rs:56-73, 88-105, 117-129`

**Problem**: No way to detect dead connections until send fails.

**Solution**: Implement ping/pong in WebSocket handlers:

```rust
async fn stream_cpu(
    mut socket: WebSocket,
    repo: Arc<SysinfoRepo>,
    interval_ms: u64,
) -> anyhow::Result<()> {
    tracing::info!("Client connected to CPU stream");
    let mut tick = tokio::time::interval(tokio::time::Duration::from_millis(interval_ms));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    
    let mut ping_interval = tokio::time::interval(Duration::from_secs(30));
    
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let stats = repo.get_cpu_stats();
                let json = serde_json::to_string(&stats)?;
                if socket.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
            _ = ping_interval.tick() => {
                if socket.send(Message::Ping(vec![])).await.is_err() {
                    break;
                }
            }
        }
    }
    Ok(())
}
```

**Impact**: Medium - Improves connection reliability

---

#### 8.2 No backpressure handling

**Location**: WebSocket send operations in `src/routes.rs`

**Problem**: If a WebSocket client is slow, sends can block the entire stream.

**Solution**: Options:
1. Use `try_send` with timeout
2. Spawn send operations as separate tasks
3. Use bounded buffer per client

**Impact**: Low-Medium - Depends on client behavior

---

#### 8.3 No message compression

**Problem**: Full system snapshots can be large (containers, storage, network data).

**Solution**: Enable WebSocket compression in axum configuration.

**Impact**: Low - Nice to have for bandwidth optimization

---

## 9. Security

#### 9.1 No authentication/authorization

**Location**: `src/routes.rs` - All endpoints are public

**Problem**: Anyone who can reach the server can connect to WebSocket endpoints and view system stats.

**Solution**: Add authentication middleware:

Options:
1. Bearer token authentication
2. API key in header or query parameter
3. mTLS (mutual TLS)
4. Integration with existing auth system

Example with API key:

```rust
async fn auth_middleware(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let api_key = headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    if api_key != std::env::var("API_KEY").unwrap_or_default() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    Ok(next.run(request).await)
}
```

**Impact**: Critical - Security requirement for production

---

#### 9.2 No rate limiting

**Problem**: Single client can open many connections and exhaust resources.

**Solution**: Add rate limiting with `tower` middleware:

```rust
use tower::ServiceBuilder;
use tower_http::limit::RequestBodyLimitLayer;

Router::new()
    .layer(ServiceBuilder::new()
        .layer(tower_governor::GovernorLayer {
            // Rate limit configuration
        })
    )
```

**Impact**: High - DoS protection

---

#### 9.3 Docker socket access

**Location**: `docker-compose.yml:20` - Full Docker socket mounted

**Problem**: Container has full Docker access via socket, can control host.

**Mitigation Options**:
1. Run with minimal privileges
2. Use Docker API over TCP with TLS instead of socket
3. Use Docker socket proxy with restricted API access
4. Document security implications

**Impact**: Medium-High - Deployment security consideration

---

#### 9.4 No HTTPS/TLS

**Problem**: WebSocket connections are unencrypted (ws:// not wss://).

**Solution**: Either:
1. Add TLS support to axum
2. Run behind reverse proxy (nginx, traefik) with TLS termination

**Impact**: High - Required for production over public networks

---

#### 9.5 CORS configuration missing

**Location**: `Cargo.toml:21` has `tower-http` with CORS feature but it's not used

**Problem**: No CORS configuration means browser clients may fail.

**Solution**: Add CORS middleware:

```rust
use tower_http::cors::{CorsLayer, Any};

Router::new()
    .layer(CorsLayer::new()
        .allow_origin(Any)  // or specific origins
        .allow_methods(Any)
        .allow_headers(Any)
    )
```

**Impact**: Medium - Required for browser clients

---

## 10. Code Quality

#### 10.1 Magic numbers

**Location**: Multiple locations

Examples:
- `src/main.rs:15` - broadcast channel capacity (16)
- `src/history_repo.rs:8` - seven days in milliseconds
- `src/worker.rs:12` - prune interval (3600)

**Solution**: Use named constants or configuration:

```rust
const BROADCAST_CHANNEL_CAPACITY: usize = 16;
const RETENTION_DAYS: u64 = 7;
const PRUNE_INTERVAL_SECONDS: u64 = 3600;
```

**Impact**: Low - Code readability

---

#### 10.2 Missing documentation

**Problem**: No rustdoc comments for public APIs.

**Solution**: Add documentation:

```rust
/// Repository for system information using the sysinfo crate.
/// 
/// Provides access to CPU, RAM, storage, network, and system statistics.
/// Thread-safe and can be shared across async tasks using Arc.
pub struct SysinfoRepo {
    sys: Arc<std::sync::Mutex<System>>,
}

impl SysinfoRepo {
    /// Creates a new SysinfoRepo and performs initial system refresh.
    pub fn new() -> Self {
        // ...
    }
    
    /// Returns current CPU statistics including usage and core counts.
    ///
    /// # Note
    /// This method blocks for MINIMUM_CPU_UPDATE_INTERVAL to get accurate readings.
    pub fn get_cpu_stats(&self) -> CpuStats {
        // ...
    }
}
```

**Impact**: Medium - Developer experience

---

#### 10.3 Inconsistent naming

**Location**: `src/routes.rs:29`

**Problem**: Health endpoint returns "Ktor: Hello from Rust homeserver!" (mentions Kotlin framework).

```rust
.route("/", get(|| async { "Ktor: Hello from Rust homeserver!" }))
```

**Solution**: Update message:

```rust
.route("/", get(|| async { "Homeserver Rust API v0.1.0" }))
```

**Impact**: Low - Cosmetic

---

#### 10.4 Unused config fields

**Location**: `src/config.rs:20, 32`

**Problem**: `database.max_pool_size` and `monitoring.sample_interval_ms` are loaded but not used.

**Solution**: Either use them or remove from config structure:

```rust
// Use sample_interval_ms in worker::spawn
// Remove max_pool_size if not implementing connection pooling
```

**Impact**: Low - Config accuracy

---

## 11. Deployment & Operations

#### 11.1 No proper health check endpoint

**Location**: `src/routes.rs:29`

**Problem**: Root returns string but doesn't check dependencies.

**Solution**: Add `/health` endpoint with dependency checks:

```rust
#[derive(Serialize)]
struct HealthResponse {
    status: String,
    database: String,
    docker: String,
    uptime_seconds: u64,
}

async fn health_check(
    State(state): State<AppState>,
) -> Json<HealthResponse> {
    let db_status = state.history_repo.ping()
        .map(|_| "ok")
        .unwrap_or("error");
    
    let docker_status = state.docker_repo.ping().await
        .map(|_| "ok")
        .unwrap_or("error");
    
    Json(HealthResponse {
        status: "ok",
        database: db_status.into(),
        docker: docker_status.into(),
        uptime_seconds: get_uptime(),
    })
}
```

**Impact**: High - Required for orchestration (Kubernetes, Docker Swarm)

---

#### 11.2 No readiness vs liveness distinction

**Problem**: No separate readiness endpoint for deployment orchestration.

**Solution**: Add both endpoints:
- `/live` - Process is running (always returns 200 if process is up)
- `/ready` - Dependencies are up and service can accept traffic

**Impact**: Medium - Kubernetes best practice

---

#### 11.3 No signal handling for graceful shutdown

**Location**: `src/main.rs:34-36`

**Problem**: No SIGTERM handling means Kubernetes/Docker kills forcefully after timeout.

**Solution**: Implement graceful shutdown:

```rust
use tokio::signal;

#[tokio::main]
async fn main() -> Result<()> {
    // ... existing setup ...
    
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    
    let worker_handle = worker::spawn(
        sysinfo_repo.clone(),
        docker_repo.clone(),
        history_repo.clone(),
        tx.clone(),
        app_config.database.flush_rate,
        shutdown_rx,
    );
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Listening on http://{}", addr);
    
    let server = axum::serve(listener, app);
    
    tokio::select! {
        result = server => {
            result?;
        }
        _ = signal::ctrl_c() => {
            tracing::info!("Received shutdown signal, draining...");
            let _ = shutdown_tx.send(());
            worker_handle.await?;
        }
    }
    
    Ok(())
}
```

**Impact**: High - Prevents data loss and improves deployment experience

---

#### 11.4 No structured logging

**Problem**: Logs are unstructured text, making them hard to query.

**Solution**: Use structured logging with tracing:

```rust
tracing::info!(
    event = "client_connected",
    endpoint = "cpu",
    "Client connected to CPU stream"
);
```

**Impact**: Medium - Improves observability

---

#### 11.5 No version endpoint

**Problem**: No way to determine running version.

**Solution**: Add version endpoint:

```rust
.route("/version", get(|| async {
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "build_time": env!("BUILD_TIME"),  // from build.rs
        "git_hash": env!("GIT_HASH"),      // from build.rs
    }))
}))
```

**Impact**: Low - Operational convenience

---

## 12. Docker & Container

#### 12.1 Dockerfile layer caching optimization

**Location**: `Dockerfile:8-10`

**Problem**: Dummy build approach for caching dependencies is hacky.

```dockerfile
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release || true
```

**Solution**: Use `cargo-chef` for proper layer caching:

```dockerfile
FROM rust:1.87 AS chef
RUN cargo install cargo-chef
WORKDIR /build

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
# ... rest unchanged ...
```

**Impact**: Medium - Faster builds

---

#### 12.2 Missing Cargo.lock in Dockerfile

**Location**: `Dockerfile:8`

**Problem**: `Cargo.lock*` glob may not work if Cargo.lock doesn't exist.

**Solution**: Explicitly copy Cargo.lock:

```dockerfile
COPY Cargo.toml Cargo.lock ./
```

Add Cargo.lock to git if not already tracked.

**Impact**: Low - Build reproducibility

---

#### 12.3 No multi-arch support

**Problem**: Dockerfile only builds for host architecture.

**Solution**: Use Docker buildx for multi-arch builds:

```dockerfile
# Add platform args
ARG TARGETPLATFORM
ARG BUILDPLATFORM

# Use cross-compilation if needed
```

**Impact**: Low - Depends on deployment needs

---

#### 12.4 Large runtime image

**Problem**: Using debian:bookworm-slim as runtime, could be smaller.

**Solution**: Consider alternatives:
1. `gcr.io/distroless/cc-debian12` (smaller, more secure)
2. Alpine Linux (smallest, but may have compatibility issues)

**Impact**: Low - Image size optimization

---

## 13. Additional Improvements

#### 13.1 Add Clippy lints

**Solution**: Add to `Cargo.toml`:

```toml
[lints.clippy]
pedantic = "warn"
unwrap_used = "warn"
expect_used = "warn"
```

**Impact**: Medium - Code quality enforcement

---

#### 13.2 Add CI/CD

**Problem**: No automated testing or building.

**Solution**: Add `.github/workflows/rust-ci.yml`:

```yaml
name: Rust CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: cargo test --all-features
      - run: cargo clippy -- -D warnings
      - run: cargo fmt -- --check
```

**Impact**: High - Prevents regressions

---

#### 13.3 Add benchmarks

**Problem**: No performance benchmarks.

**Solution**: Add criterion benchmarks:

```toml
[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "stats_benchmark"
harness = false
```

**Impact**: Low - Performance tracking

---

#### 13.4 Add API documentation

**Problem**: No OpenAPI/Swagger docs for HTTP endpoints.

**Solution**: Add `utoipa` crate for OpenAPI generation.

**Impact**: Low - Developer experience

---

## Summary: Priority Matrix

### 🔴 Critical (Must Fix Before Production)

| Priority | Issue | Location | Impact |
|----------|-------|----------|--------|
| 1 | No authentication/authorization | routes.rs | Security |
| 2 | Blocking operations in async | sysinfo_repo.rs:20-24 | Performance |
| 3 | Unwrap usage (panic risk) | Multiple files | Stability |
| 4 | No graceful shutdown | main.rs:36 | Data loss risk |
| 5 | No actual tests | tests/ | Code quality |

---

### 🟡 High Priority (Fix Soon)

| Priority | Issue | Location | Impact |
|----------|-------|----------|--------|
| 6 | No rate limiting | routes.rs | Security |
| 7 | Docker stats cleanup | docker_repo.rs:81-92 | Resource leak |
| 8 | Health check endpoint | routes.rs | Operations |
| 9 | Config validation | config.rs:35-42 | Robustness |
| 10 | Worker tick logic | worker.rs:55-65 | Correctness |

---

### 🟢 Medium Priority (Should Fix)

| Priority | Issue | Location | Impact |
|----------|-------|----------|--------|
| 11 | Add metrics/observability | N/A | Operations |
| 12 | WebSocket ping/pong | routes.rs | Reliability |
| 13 | Broadcast capacity | main.rs:15 | Reliability |
| 14 | Optimize sysinfo refresh | sysinfo_repo.rs | Performance |
| 15 | Add CORS configuration | routes.rs | Browser support |
| 16 | Use async Mutex | sysinfo_repo.rs:8 | Async compat |
| 17 | Structured logging | Multiple | Observability |

---

### ⚪ Low Priority (Nice to Have)

| Priority | Issue | Location | Impact |
|----------|-------|----------|--------|
| 18 | Add documentation | All files | Developer UX |
| 19 | Fix magic numbers | Multiple | Readability |
| 20 | Optimize Dockerfile | Dockerfile | Build speed |
| 21 | Add CI/CD | N/A | Automation |
| 22 | WebSocket compression | routes.rs | Bandwidth |
| 23 | Version endpoint | routes.rs | Operations |
| 24 | Add benchmarks | N/A | Performance tracking |

---

## Implementation Roadmap

### Phase 1: Critical Fixes (Week 1)
1. Remove all `.unwrap()` calls and add proper error handling
2. Fix blocking operations in async context (sysinfo)
3. Implement graceful shutdown with signal handling
4. Add basic authentication (API key)
5. Write essential unit tests

### Phase 2: Production Hardening (Week 2)
6. Add rate limiting middleware
7. Fix Docker stats stream cleanup
8. Implement health/ready/live endpoints
9. Add config validation
10. Fix worker tick counters
11. Add integration tests

### Phase 3: Observability (Week 3)
12. Add Prometheus metrics
13. Implement WebSocket ping/pong
14. Add structured logging
15. Increase broadcast channel capacity
16. Add CORS support

### Phase 4: Optimization (Week 4)
17. Convert to async Mutex where appropriate
18. Optimize sysinfo refresh patterns
19. Add comprehensive documentation
20. Optimize Dockerfile with cargo-chef
21. Set up CI/CD pipeline

---

## Conclusion

The Rust homeserver is a solid foundation with clean architecture and good library choices. The main concerns are:

1. **Stability**: Unwrap usage and blocking operations need immediate attention
2. **Security**: No auth/rate limiting makes it unsuitable for production
3. **Testing**: Zero test coverage is a significant risk
4. **Operations**: Missing health checks and graceful shutdown

Once these issues are addressed, this will be a robust, production-ready monitoring service with excellent performance characteristics.

**Overall Assessment**: 7/10 (Good foundation, needs production hardening)

---

**Next Steps**: Review this document and prioritize which items to tackle first. I recommend starting with the Critical fixes (Priority 1-5) before any production deployment.
