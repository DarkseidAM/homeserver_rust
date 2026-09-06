# Changelog

All notable changes to this project will be documented in this file.

## [0.10.1] - 2026-09-06

### CI/CD

- **release:** Use git-cliff --latest to generate notes only for current release

### Performance

- Sysfs caching, database pragmas, incremental vacuum, and collect_all consolidation

## [0.10.0] - 2026-09-05

### Bug Fixes

- **db:** Wire max_pool_size into SqlitePoolOptions and reduce default to 5

### CI/CD

- Fetch tags and pass explicit tag to git cliff in release workflow

### Documentation

- Sync CODEBASE.md and README.md with health_probe module and performance updates

### Features

- **cli:** Add --health probe and drop curl from docker runtime image

### Other

- Ignore docs/superpowers/ to keep superpower docs local

### Performance

- Initialize sysinfo lazily without eager process table loading
- Constrain tokio runtime to 2 worker threads
- **ws:** Broadcast pre-serialized Arc<str> snapshots to eliminate redundant JSON encoding
- **config:** Reduce broadcast_capacity default to 16
- **docker:** Enforce 128M/0.5 CPU resource limits and jemalloc 5s dirty decay

### Release

- Bump version to 0.10.0

## [0.9.1] - 2026-09-05

### Bug Fixes

- **ci:** Ensure lowercase repository name in Docker image references

### CI/CD

- Grant checks write permission to audit job
- Use native ARM64 and AMD64 matrix runners for Docker builds
- Optimize security audit with pre-built binary and fix docker cache permissions
- Switch Docker build cache to GHCR registry cache

### Performance

- Add optimized release profile with fat LTO and abort panic

### Release

- Bump version to 0.9.1

## [0.9.0] - 2026-06-30

### CI/CD

- Trigger checks for phase5 against main

## [0.8.0] - 2026-06-25

### Bug Fixes

- **http:** Guard /api/history against i64 overflow/underflow on extreme bounds

### CI/CD

- **docs:** Add workflow_dispatch trigger
- **cd:** Multi-arch (amd64+arm64) images and RustSec dependency audit

### Documentation

- Format CI audit step to lead with `cargo audit` for consistency

### Features

- **homeserver:** V0.8.0 — monitoring metrics, SQLite schema versioning ([#58](https://github.com/DarkseidAM/homeserver_rust/pull/58))
- Agent map, rustdoc on Pages, automated releases ([#71](https://github.com/DarkseidAM/homeserver_rust/pull/71))
- Add graphify knowledge graph (416 nodes, 917 edges, 25 communities)
- **history:** Full CPU/RAM fidelity + additive schema migrations
- **ws,ops:** Permessage-deflate via yawc, /health endpoint, gosu shutdown
- **gpu:** NVIDIA(NVML)+AMD/Intel(sysfs) GPU metrics, schema v4
- **smart:** SMART disk health via smartctl, schema v5
- **alerting:** Threshold rules with tracing + webhook notifications

### Other

- Add graphify-rs MCP server config and enable it for Claude Code
- **graphify:** Refresh knowledge graph for Phase 5 (gpu/smart/alerting)
- **graphify:** Refresh knowledge graph for Phase 5 test additions
- **graphify:** Refresh knowledge graph for phase5
- **release:** Bump version to 0.9.0

### Performance

- Phase 1 correctness & performance hardening
- **gpu:** Offload collect to spawn_blocking; avoid clone in sort; strip_prefix

### Testing

- **phase5:** GPU/SMART persistence round-trips, persist gating, aggregation carry-over

## [0.7.1] - 2026-06-21

### Bug Fixes

- Normalize 5-field Unix cron to 6-field for vacuum_schedule
- Dockerfile to reduce vulnerabilities ([#24](https://github.com/DarkseidAM/homeserver_rust/pull/24))
- **deps:** Update rust crate wincode to 0.5 ([#47](https://github.com/DarkseidAM/homeserver_rust/pull/47))
- **deps:** Update rust crate cron to 0.16 ([#46](https://github.com/DarkseidAM/homeserver_rust/pull/46))
- **deps:** Update rust crate sysinfo to 0.39 ([#56](https://github.com/DarkseidAM/homeserver_rust/pull/56))
- **deps:** Update bollard to 0.21, migrate imports to bollard::models
- **deps:** Update rust-dependencies-minor ([#63](https://github.com/DarkseidAM/homeserver_rust/pull/63))
- **deps:** Update rust crate tikv-jemallocator to 0.7 ([#64](https://github.com/DarkseidAM/homeserver_rust/pull/64))
- **deps:** Update rust-dependencies-minor to 0.7 ([#68](https://github.com/DarkseidAM/homeserver_rust/pull/68))
- **deps:** Update rust-dependencies-minor ([#70](https://github.com/DarkseidAM/homeserver_rust/pull/70))

### Other

- Configure Renovate with dependency grouping, automated maintenance, and selective automerge rules

## [0.7.0] - 2026-02-11

### Bug Fixes

- Fix: duration-based timers, cron VACUUM, background history writer
  - Worker: stats log and prune use real intervals (no tick coupling)
  - Config: prune_interval_secs, vacuum_schedule (cron), vacuum_interval_secs
  - Aggregation worker: VACUUM on cron (local time) or fixed interval
  - History writer: dedicated task via channel; flush by count/interval/shutdown
  - Config: flush_interval_secs for writer; collapsible_if clippy fix

## [0.6.2] - 2026-02-11

### Bug Fixes

- Jemalloc compilation issues

## [0.6.1] - 2026-02-11

### Bug Fixes

- Stale process stats and add jemallocator for memory efficiency

## [0.6.0] - 2026-02-08

### Features

- Downsampling, history API, backfill, vacuum, and resilient blob parsing

## [0.5.0] - 2026-02-06

## [0.4.0] - 2026-02-05

## [0.3.0] - 2026-02-04

## [0.2.0] - 2026-02-04

## [0.1.0] - 2026-02-02


