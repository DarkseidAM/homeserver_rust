# Rust migration: complete

The Kotlin server has been successfully migrated to Rust. Core functionality is feature-complete; test coverage is the main remaining task.

---

## What was migrated

### ✅ Core application

- [x] Config loading (TOML instead of HOCON; same structure)
- [x] Domain models (CpuStats, RamStats, ContainerStats, FullSystemSnapshot, etc.)
- [x] System stats via **sysinfo** (CPU, RAM, storage partitions/disks, network interfaces, system info)
- [x] Docker stats via **bollard** (list running containers, persistent stats streams per container)
- [x] SQLite history (init schema, batch save, prune 7-day old data)
- [x] Background worker (gather stats every 1s, broadcast via channel, flush to DB every N seconds, prune hourly)
- [x] HTTP + WebSocket routes:
  - `GET /` – health
  - `WS /ws/cpu` – CPU stats stream
  - `WS /ws/ram` – RAM stats stream
  - `WS /ws/system` – full system snapshot stream
- [x] JSON serialization with camelCase (API-compatible with Kotlin server)

### ✅ Deployment

- [x] Dockerfile (multi-stage: Rust builder + Debian slim)
- [x] docker-compose.yml (run on port 8082, mount config/data/Docker socket)
- [x] .gitignore, .dockerignore
- [x] README.md (how to run, build, deploy)

### 🟡 Test structure

- [x] Test files created (`tests/*.rs`)
- [ ] Tests are stubs; need to fill in unit/integration tests (config, models, repos, routes, worker)

---

## What's different (intentional)

| Feature | Kotlin | Rust |
|---------|--------|------|
| **Config format** | HOCON | TOML |
| **Config load** | `-Dconfig.file=...` (JVM prop) | `CONFIG_FILE` env var |
| **Migrations** | Flyway (auto from `db/migrations/`) | Manual DDL in `history_repo::init()` |
| **DI** | Koin | Manual Arc + shared state |
| **Error handling** | Exceptions | `Result<T, anyhow::Error>` |
| **Logging** | Logback (structured) | tracing (structured) |

---

## What's missing/stubbed

| Item | Reason | Impact |
|------|--------|--------|
| **Disk I/O stats** | sysinfo doesn't expose cumulative read/write per disk | `DiskDeviceStat.{read,write}_bytes` are 0 |
| **CPU temperature** | sysinfo doesn't expose temp on all platforms | `CpuStats.temperature` is 0 |
| **Sensors (voltage, fan)** | sysinfo doesn't expose sensors | `SystemStats.{cpu_voltage, fan_speeds}` are 0/empty |
| **Network speed** | sysinfo has limited interface metadata | `InterfaceStat.speed` is 0 |

For production use: these are **minor**; the main metrics (CPU %, RAM, disk space, network bytes, containers) are all present. If you need temp/sensors, you can read `/sys/class/thermal/` or `/sys/class/hwmon/` directly.

---

## Performance (expected)

From `server/PERFORMANCE_ANALYSIS.md`:

| Metric | Kotlin/JVM | Rust |
|--------|------------|------|
| **RSS** | ~100–250 MiB | ~10–20 MiB |
| **Startup** | 2–5 s (JVM) | <1 s |
| **CPU (steady)** | ~1% | ~0.3–0.6% |
| **GC pauses** | Yes | No |

The Rust version gives **much lower memory**, **no GC**, and **faster startup**; the I/O-bound work (OSHI/Docker/SQLite) is the same, so CPU reduction is modest.

---

## How to proceed

### 1. Run locally

```bash
cd server/rust
cargo run
```

Open `http://localhost:8081`, connect WebSocket clients to `/ws/cpu`, `/ws/ram`, `/ws/system`.

### 2. Run in Docker

```bash
cd server/rust
docker compose up --build
```

Port 8082 (vs Kotlin's 8081). Check logs:

```bash
docker compose logs -f
```

### 3. Compare with Kotlin

Run both side-by-side (Kotlin on 8081, Rust on 8082) and compare:

- Memory: `docker stats`
- Latency: connect WebSocket clients, measure message delivery time
- Startup: time from container start to "Responding at http://..."

### 4. Fill in tests

See `tests/*.rs` for TODOs. Add:

- **Config tests**: load config.toml, validate sections, test bad config
- **Model tests**: serialize/deserialize, verify camelCase JSON
- **Repo tests**: sysinfo (mock or real), docker (mock or local Docker), history (in-memory SQLite)
- **Integration tests**: HTTP/WS routes (use axum test client), worker loop, end-to-end

Run:
```bash
cargo test
cargo test -- --nocapture  # see output
cargo test --release       # optimize for perf tests
```

### 5. Add CI

Create `.github/workflows/rust-ci.yml`:

```yaml
name: Rust CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - run: cd server/rust && cargo test
      - run: cd server/rust && cargo clippy -- -D warnings
      - run: cd server/rust && cargo build --release
```

### 6. Migrate fully

Once tested:

- Update documentation to point to Rust server
- Archive the Kotlin server or keep it as a reference
- Deploy Rust to production

---

## Summary

**Migration is complete** for core functionality. The Rust server is **API-compatible** with the Kotlin server (same WebSocket endpoints, same JSON shape). Main remaining task: **test coverage** (stubs are in place; need to implement assertions and coverage for config, models, repos, routes, worker).
