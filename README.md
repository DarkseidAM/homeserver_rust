# Homeserver (Rust port)

Rust port of the Kotlin server: system and Docker stats over WebSockets, SQLite history.

## Run

```bash
cd server/rust
cargo run
```

Config: `config.toml` in the current directory, or set `CONFIG_FILE` to a path.

## Endpoints

- `GET /` – health
- `WS /ws/cpu` – CPU stats stream (interval from `publishing.cpu_stats_frequency_ms`)
- `WS /ws/ram` – RAM stats stream (interval from `publishing.ram_stats_frequency_ms`)
- `WS /ws/system` – full system snapshot stream (CPU, RAM, containers, storage, network, system)

## Config (`config.toml`)

Same semantics as Kotlin `server/config/application.conf`:

- `server.port`, `server.host`
- `database.path`, `database.flush_rate`
- `publishing.cpu_stats_frequency_ms`, `ram_stats_frequency_ms`

## Build release

```bash
cargo build --release
./target/release/homeserver
```

## Docker

To run in Docker (Unix socket for Docker required):

```bash
docker build -f Dockerfile ..   # from server/rust, context = server
# or add a Dockerfile in server/rust that builds the binary and copy config
```

## Differences from Kotlin server

- Storage and network stats are stubbed (empty) for now; sysinfo 0.31 disk/network APIs can be wired later.
- Config is TOML; Kotlin uses HOCON.
- JSON field names use `snake_case` (Rust/serde default); Kotlin uses `camelCase`. Clients that expect camelCase may need to accept both or use `#[serde(rename = "...")]` for compatibility.
