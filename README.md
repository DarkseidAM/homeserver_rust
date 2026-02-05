# Homeserver (Rust)

A robust, efficient, and modern system monitoring agent and dashboard backend written in Rust. It serves as a direct, high-performance replacement for the legacy Kotlin-based homeserver.

![Version](https://img.shields.io/badge/version-0.5.0-blue.svg)
![License](https://img.shields.io/badge/license-MIT-green.svg)

## Features

*   **Real-time Monitoring**: Streams CPU, RAM, Disk, Network, and System stats via WebSockets.
*   **Docker Integration**: Auto-discovers running containers and streams per-container metrics (CPU, Memory, I/O, Network) in real-time.
*   **Historical Data**: Persists system snapshots to a local SQLite database for historical graphing.
*   **Efficient Architecture**:
    *   **Async Core**: Built on Tokio and Axum for high concurrency.
    *   **Non-Blocking**: Optimized CPU sampling logic to prevent blocking the runtime.
    *   **Binary Storage**: Uses `wincode` binary serialization for database efficiency, reducing storage footprint by ~50% compared to JSON.
    *   **WAL Mode**: SQLite configured in Write-Ahead Logging mode for better concurrent performance.
*   **Portable Deployment**:
    *   **Dynamic Permissions**: Smart entrypoint script automatically detects Docker socket GID, ensuring it runs on any Linux host without permission errors.
    *   **Multi-Arch**: Dockerfile supports building small, secure images (Debian-slim base).

## Architecture

*   **Runtime**: [Tokio](https://tokio.rs/) (Async I/O)
*   **Web Framework**: [Axum](https://github.com/tokio-rs/axum) (HTTP & WebSockets)
*   **Database**: [SQLx](https://github.com/launchbadge/sqlx) (SQLite)
*   **System Info**: `sysinfo` crate + custom Linux `/proc` parsing.
*   **Docker**: `bollard` crate (Docker API).
*   **Serialization**: `serde` (API JSON) + `wincode` (DB BLOBs).

## Configuration (`config.toml`)

```toml
[server]
port = 8081
host = "0.0.0.0"

[database]
path = "data/server.db"
max_pool_size = 10
flush_rate = 10        # Flush to DB every N ticks
retention_days = 3     # Prune history older than N days

[publishing]
cpu_stats_frequency_ms = 1000
ram_stats_frequency_ms = 1000
broadcast_capacity = 60

[monitoring]
sample_interval_ms = 1000
stats_log_interval_secs = 60
```

## Deployment

### Option 1: Pre-built Image from GitHub Container Registry (Recommended)

Use the production-ready deployment files in the `deployment/` folder:

```bash
cd deployment
cp .env.example .env
# Edit .env and docker-compose.yml to customize
docker compose up -d
```

See [`deployment/README.md`](./deployment/README.md) for detailed instructions.

### Option 2: Build from Source

For development or custom builds:

```bash
docker compose up -d
```

**Note:** Both methods mount `/var/run/docker.sock` so the container can monitor other containers. The entrypoint script (`docker-entrypoint.sh`) automatically handles the permissions, so **no manual GID configuration is needed**.

## Development

1.  **Prerequisites**: Rust 1.93+, Docker (optional, for container stats).
2.  **Build**:
    ```bash
    cargo build --release
    ```
3.  **Run**:
    ```bash
    cargo run
    ```
4.  **Tests**:
    ```bash
    cargo test
    ```

## Database Schema & Migrations

The application uses a **Blob-based History** approach:
*   **Table `system_history`**: Stores `created_at` (timestamp), `cpu_load`, `memory_used`, and several BLOB columns (`container_data`, `storage_data`, etc.).
*   **Serialization**: Complex nested objects (like container lists) are serialized into binary (`wincode`) before storage.
*   **Versioning**: Data blobs are prefixed with a version byte. This allows the application to "read-repair" or adapt old data formats on the fly without requiring complex SQL migration scripts for the binary content.

## License

MIT
