# Homeserver-Rust: Code Analysis & Netdata Comparison

## Current State Summary

Rust-based system monitoring agent (v0.7.1) that collects real-time metrics and stores them in SQLite. Well-architected with async Tokio, Axum for HTTP/WebSocket, and efficient binary serialization (wincode). Clean module separation, proper error handling, and production-ready Docker deployment.

### What's Implemented

| Area | Status | Notes |
|------|--------|-------|
| CPU monitoring | Done | Usage %, cores, model |
| RAM monitoring | Done | Total, used, available, % |
| Storage/Disk | Partial | Partitions OK; disk I/O stats are **hardcoded to 0** |
| Network | Done | Interfaces, IPs, throughput per second |
| Docker containers | Done | CPU, mem, net I/O, block I/O, PIDs, throttling |
| Data persistence | Done | SQLite with WAL, binary serialization |
| Data aggregation | Done | Raw → 1min → 5min downsampling |
| WebSocket streaming | Done | Real-time CPU, RAM, full system |
| Historical API | Done | Time range queries with resolution |
| Docker deployment | Done | Multi-stage builds, entrypoint with dynamic GID |

---

## Feature Gap Analysis vs Netdata

### 1. Per-Core CPU Metrics

Only `global_cpu_usage()` is reported as a single percentage. Netdata shows **per-core utilization** (user, system, nice, iowait, irq, softirq, steal, guest). The `sysinfo` crate provides per-CPU data via `sys.cpus()` but only `sys.global_cpu_usage()` is used.

### 2. CPU Temperature (stubbed out)

In `get_cpu_stats()`, temperature is hardcoded to `0.0`. Netdata reads from `hwmon`/`thermal_zone`. Could read `/sys/class/thermal/thermal_zone*/temp` or `/sys/class/hwmon/hwmon*/temp*_input` directly.

### 3. Disk I/O Statistics (stubbed out)

In `get_storage_stats()`, the `DiskDeviceStat` fields are hardcoded:

```rust
model: String::new(),      // always empty
read_bytes: 0,             // always 0
write_bytes: 0,            // always 0
transfer_time_ms: 0,       // always 0
```

Netdata reads `/proc/diskstats` for reads/writes/IOPS/utilization/queue depth per device. This is a significant gap.

### 4. Swap Memory (missing entirely)

Netdata monitors swap usage (total, used, free, swap in/out). The `sysinfo` crate provides `sys.total_swap()`, `sys.used_swap()`, `sys.free_swap()` — just not collected.

### 5. Load Average (missing)

Netdata shows 1/5/15 minute load averages. Available from `/proc/loadavg` or `sysinfo`.

### 6. Memory Breakdown (missing detail)

Netdata shows buffers, cached, slab, page tables, kernel stack, etc. Only total/used/available is tracked. Linux-specific detail from `/proc/meminfo` would be needed.

### 7. Process Monitoring (minimal)

Only `process_count` and `thread_count` globally. Netdata provides:
- Per-process CPU/memory/I/O
- Top-N processes by resource usage
- Process groups/categories (e.g., "web servers", "databases")
- OOM kill tracking

### 8. Network Error/Drop Stats (missing for host)

Errors/drops are tracked for Docker containers but NOT for host network interfaces. The `sysinfo` crate doesn't provide errors/drops, so `/proc/net/dev` would need to be parsed directly.

### 9. Alerting / Health Checks (completely absent)

Netdata has a full alerting system:
- Threshold-based alerts (CPU > 90% for 5 min)
- Anomaly detection
- Notification channels (email, Slack, PagerDuty, Telegram, webhooks)
- Alert templates and silencing

Zero alerting capability exists.

### 10. Prometheus / OpenMetrics Export (missing)

Netdata exposes a `/api/v1/allmetrics?format=prometheus` endpoint. Critical for integration with the broader monitoring ecosystem (Grafana, Alertmanager, etc.). The API is custom JSON only.

### 11. System Services / Systemd Monitoring (missing)

Netdata monitors systemd services (active/failed/inactive counts). Useful for homeserver setups.

### 12. Log Monitoring (missing)

Netdata can parse and monitor system logs (systemd journal, `/var/log/syslog`). Error rate tracking, pattern matching.

### 13. Application-Specific Collectors (missing)

Netdata has 800+ collectors. Key ones for a homeserver:
- **Nginx/Apache/Caddy** metrics
- **MySQL/PostgreSQL/Redis** metrics
- **Pi-hole** metrics
- **Certs/SSL** expiry monitoring
- **DNS query** stats
- **SMART** disk health
- **UPS/battery** status
- **IPMI** sensors

### 14. Multi-Node / Streaming (missing)

Netdata supports parent-child streaming where multiple nodes push metrics to a central parent. This setup is single-node only.

### 15. Dashboard / UI (missing entirely)

Netdata ships with a complete web dashboard. This project is API-only — no UI at all.

### 16. Authentication & Authorization (missing)

All endpoints are public. No API keys, no basic auth, no JWT, no RBAC. Anyone who can reach port 8081 has full access.

### 17. GPU Monitoring (missing)

Netdata monitors NVIDIA GPUs (utilization, temperature, memory, power). Relevant if the server has a GPU.

### 18. Filesystem Monitoring (missing detail)

Beyond disk space, Netdata tracks:
- Inode usage
- Mount point health/staleness
- NFS stats
- ZFS pool health (relevant for homeservers)

### 19. Interrupt and Softirq Stats (missing)

Netdata monitors `/proc/interrupts` and `/proc/softirqs` — useful for debugging performance issues.

### 20. Entropy / Random Pool (missing)

Tracks `/proc/sys/kernel/random/entropy_avail` — important for servers doing encryption.

---

## Portainer & External Tool Integration Opportunities

### Portainer Integration

Portainer exposes a REST API that gives much richer Docker/container management data than raw Docker socket:

| Feature | How | Value |
|---------|-----|-------|
| **Container lifecycle** | `POST /api/endpoints/{id}/docker/containers/{id}/start\|stop\|restart` | Start/stop/restart containers from the dashboard |
| **Container logs** | `GET /api/endpoints/{id}/docker/containers/{id}/logs` | Stream container logs alongside metrics |
| **Docker Compose stacks** | `GET /api/stacks` | Show stack health, deploy/redeploy stacks |
| **Image management** | `GET /api/endpoints/{id}/docker/images/json` | Track image versions, detect outdated images |
| **Volume usage** | `GET /api/endpoints/{id}/docker/volumes` | Monitor volume disk usage |
| **Network topology** | `GET /api/endpoints/{id}/docker/networks` | Show which containers share networks |
| **Registry integration** | Via Portainer API | Check for newer image versions |
| **Environment management** | `GET /api/endpoints` | Multi-environment monitoring |
| **Container health** | Container health check status | Beyond just running/stopped — actual health |
| **Resource limits** | Container inspect data | Show configured vs actual CPU/memory limits |

To integrate with Portainer:
- A new config section for Portainer API URL + API key
- A `portainer_repo` module similar to `docker_repo`
- New API endpoints to proxy Portainer actions
- New models for stacks, images, volumes

### Prometheus Metrics Export

Adding a `GET /metrics` endpoint in Prometheus format would:
- Connect Grafana for advanced visualization
- Use Alertmanager for alerting
- Join the existing monitoring ecosystem
- This is probably the **single highest-value integration** possible

### Traefik Integration

The production docker-compose already has Traefik labels. Could:
- Read Traefik's API (`/api/http/routers`, `/api/http/services`)
- Show active routes, TLS certificate status
- Track request rates and error rates per service

### SMART Disk Health (via `smartctl`)

- Run `smartctl -a /dev/sdX --json` periodically
- Parse temperature, reallocated sectors, power-on hours
- Predict disk failure before it happens
- Critical for a homeserver

### UPS Monitoring (via NUT - Network UPS Tools)

- Connect to `upsd` daemon
- Monitor battery level, load, runtime remaining
- Essential for homeservers with UPS

### Pi-hole / AdGuard Home

- Pi-hole API: `http://pi.hole/admin/api.php`
- Queries today, blocked today, top clients, top domains
- Common homeserver component

### Nginx Proxy Manager / Caddy

- Reverse proxy stats (active connections, requests/sec)
- SSL certificate expiry dates
- Upstream health status

---

## Priority Recommendations

| Priority | Feature | Effort | Impact |
|----------|---------|--------|--------|
| **P0** | Fix disk I/O stats (read `/proc/diskstats`) | Low | High — currently returning zeros |
| **P0** | Fix CPU temperature (read `/sys/class/thermal/`) | Low | High — currently returning 0.0 |
| **P1** | Add swap memory monitoring | Low | Medium |
| **P1** | Add load averages (1/5/15 min) | Low | Medium |
| **P1** | Per-core CPU stats | Low-Med | High |
| **P1** | Add Prometheus `/metrics` endpoint | Medium | Very High — unlocks entire ecosystem |
| **P2** | Add basic auth/API key auth | Medium | High — security critical |
| **P2** | Container health status (not just running/stopped) | Low | Medium |
| **P2** | SMART disk health | Medium | High for homeserver |
| **P2** | Basic alerting (thresholds + webhooks) | Medium-High | Very High |
| **P3** | Portainer API integration (stacks, images, volumes) | Medium | High |
| **P3** | Host network errors/drops from `/proc/net/dev` | Low | Medium |
| **P3** | Memory breakdown (buffers, cached, slab) | Low-Med | Medium |
| **P3** | Container logs endpoint | Medium | Medium |
| **P4** | Process top-N monitoring | Medium | Medium |
| **P4** | Pi-hole/AdGuard integration | Medium | Niche but useful |
| **P4** | Traefik/reverse proxy integration | Medium | Medium |
| **P4** | Multi-node streaming | High | Medium |

---

## Code Quality Observations

### 1. `DiskDeviceStat` is dead code

The model exists but `read_bytes`, `write_bytes`, `model`, and `transfer_time_ms` are never populated. Either implement them or remove the struct fields to avoid confusion.

### 2. CPU voltage and fan speeds are dead

`cpu_voltage: 0.0` and `fan_speeds: vec![]` are always returned. Same issue — either implement via `hwmon` parsing or remove.

### 3. `is_up` for network interfaces is always `true`

Should read the actual interface state from `/sys/class/net/<iface>/operstate`.

### 4. No graceful shutdown in Docker mode

When `in_container` is true, signal handling is skipped entirely, meaning the worker and writer don't get a clean shutdown signal. The history writer's final flush may not run.

### 5. Aggregation worker handle is immediately dropped

`drop(agg_handle)` in `main.rs` means the task can't be awaited on shutdown. The task keeps running but can't be cleanly joined.
