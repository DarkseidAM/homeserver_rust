# Homeserver (Rust)

System and Docker stats over WebSockets, SQLite history.

## Run

```bash
cargo run
```

Config: `config.toml` in the current directory, or set `CONFIG_FILE` to a path.

## Endpoints

**GET**

- `GET /` – health
- `GET /version` – service name and version
- `GET /api/info` – static system identity (OS, hostname, CPU name; fetch once)

**WebSocket**

- `WS /ws/cpu` – CPU stats stream (interval from `publishing.cpu_stats_frequency_ms`)
- `WS /ws/ram` – RAM stats stream (interval from `publishing.ram_stats_frequency_ms`)
- `WS /ws/system` – full system snapshot stream (CPU, RAM, containers, storage, network, system)

## Config (`config.toml`)

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
docker build -f Dockerfile .
docker run --rm -v /var/run/docker.sock:/var/run/docker.sock -p 8080:8080 homeserver-rust
```
