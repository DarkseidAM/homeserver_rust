# Graph Report - homeserver-rust  (2026-06-25)

## Corpus Check
- 60 files · ~24,201 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 555 nodes · 970 edges · 38 communities (29 shown, 9 thin omitted)
- Extraction: 97% EXTRACTED · 3% INFERRED · 0% AMBIGUOUS · INFERRED: 33 edges (avg confidence: 0.81)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `a61af0c5`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- [[_COMMUNITY_Community 0|Community 0]]
- [[_COMMUNITY_Community 1|Community 1]]
- [[_COMMUNITY_Community 2|Community 2]]
- [[_COMMUNITY_Community 3|Community 3]]
- [[_COMMUNITY_Community 4|Community 4]]
- [[_COMMUNITY_Community 5|Community 5]]
- [[_COMMUNITY_Community 6|Community 6]]
- [[_COMMUNITY_Community 7|Community 7]]
- [[_COMMUNITY_Community 8|Community 8]]
- [[_COMMUNITY_Community 9|Community 9]]
- [[_COMMUNITY_Community 10|Community 10]]
- [[_COMMUNITY_Community 11|Community 11]]
- [[_COMMUNITY_Community 12|Community 12]]
- [[_COMMUNITY_Community 14|Community 14]]
- [[_COMMUNITY_Community 16|Community 16]]
- [[_COMMUNITY_Community 17|Community 17]]
- [[_COMMUNITY_Community 18|Community 18]]
- [[_COMMUNITY_Community 19|Community 19]]
- [[_COMMUNITY_Community 20|Community 20]]
- [[_COMMUNITY_Community 21|Community 21]]
- [[_COMMUNITY_Community 22|Community 22]]
- [[_COMMUNITY_Community 25|Community 25]]
- [[_COMMUNITY_Community 26|Community 26]]
- [[_COMMUNITY_Community 27|Community 27]]
- [[_COMMUNITY_Community 28|Community 28]]
- [[_COMMUNITY_Community 29|Community 29]]
- [[_COMMUNITY_Community 30|Community 30]]
- [[_COMMUNITY_Community 32|Community 32]]
- [[_COMMUNITY_Community 33|Community 33]]
- [[_COMMUNITY_Community 34|Community 34]]
- [[_COMMUNITY_Community 35|Community 35]]
- [[_COMMUNITY_Community 36|Community 36]]
- [[_COMMUNITY_Community 37|Community 37]]

## God Nodes (most connected - your core abstractions)
1. `FullSystemSnapshot` - 30 edges
2. `Codebase Guide — homeserver-rust` - 19 edges
3. `AggregatedSnapshot` - 17 edges
4. `DockerRepo` - 16 edges
5. `SystemInfo` - 16 edges
6. `AppState` - 16 edges
7. `ContainerStats` - 15 edges
8. `Homeserver (Rust)` - 15 edges
9. `AppConfig` - 14 edges
10. `SysinfoRepo` - 14 edges

## Surprising Connections (you probably didn't know these)
- `Rust Agent Rules (.agents/rules/rust.md)` --references--> `main.rs Entry Point`  [INFERRED]
  .agents/rules/rust.md → CODEBASE.md
- `aggregate_snapshots_empty_returns_none()` --calls--> `aggregate_snapshots()`  [INFERRED]
  tests/aggregation_tests.rs → src/history_repo/aggregation.rs
- `aggregate_snapshots_multiple_computes_avg_min_max()` --calls--> `aggregate_snapshots()`  [INFERRED]
  tests/aggregation_tests.rs → src/history_repo/aggregation.rs
- `aggregate_snapshots_single_snapshot()` --calls--> `aggregate_snapshots()`  [INFERRED]
  tests/aggregation_tests.rs → src/history_repo/aggregation.rs
- `snapshot()` --references--> `FullSystemSnapshot`  [EXTRACTED]
  tests/aggregation_tests.rs → src/models/system.rs

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Core Data Collection and Persistence Pipeline** — codebase_sysinforepo, codebase_dockerrepo, codebase_fullsystemsnapshot, codebase_worker_mod, codebase_worker_history_writer, codebase_historyrepo, codebase_sqlite_system_history [EXTRACTED 1.00]
- **Tiered Aggregation and Downsampling Pipeline** — codebase_sqlite_system_history, codebase_aggregation_worker, codebase_sqlite_system_history_aggregated, codebase_history_repo_aggregation, codebase_history_repo_history_merge, codebase_backfillrs [EXTRACTED 1.00]
- **CI/CD Release Chain (CI → Tag → Docker + Release + Docs)** — ci_ci_yml, ci_tagversion_yml, ci_docker_yml, ci_release_yml, ci_docs_yml [EXTRACTED 1.00]

## Communities (38 total, 9 thin omitted)

### Community 0 - "Community 0"
Cohesion: 0.07
Nodes (30): Docker, DockerRepo, aggregate_aggregated_snapshots(), aggregate_containers(), aggregate_containers_from_aggregated(), aggregate_one_container(), aggregate_snapshots(), init_aggregated_table() (+22 more)

### Community 1 - "Community 1"
Cohesion: 0.09
Nodes (43): Arc, AtomicU64, AtomicUsize, minimal_snapshot(), Drop, HistoryRepo, IntoResponse, FullSystemSnapshot (+35 more)

### Community 2 - "Community 2"
Cohesion: 0.06
Nodes (47): Alerting / Health Checks (Missing), Authentication & Authorization (Missing), CPU Temperature (Stubbed Out), Disk I/O Statistics (Stubbed Out), DiskDeviceStat Dead Code Issue, Homeserver-Rust Code Analysis & Netdata Comparison, Load Average Monitoring (Missing), Netdata (+39 more)

### Community 3 - "Community 3"
Cohesion: 0.23
Nodes (15): aggregation_worker.rs, backfill.rs, config.rs AppConfig, cron crate, tikv-jemallocator crate, Docker Collector (`src/docker_repo/`), `DockerRepo`, `HistoryRepo` (+7 more)

### Community 4 - "Community 4"
Cohesion: 0.06
Nodes (5): AppConfig, MonitoringConfig, normalize_cron_expression(), PublishingConfig, ServerConfig

### Community 5 - "Community 5"
Cohesion: 0.08
Nodes (28): HashMap, disk_sysfs_base_device_name(), DiskIoRaw, parse_diskstats(), read_disk_model_linux(), read_diskstats_linux(), parse_hwmon_temp(), parse_loadavg() (+20 more)

### Community 6 - "Community 6"
Cohesion: 0.12
Nodes (16): Codebase Guide — homeserver-rust, Configuration Reference, Core Snapshot Types, Data Flow, Domain Models (`src/models/`), Entry Point (`src/main.rs`), High-Level Architecture, Key Dependencies (+8 more)

### Community 7 - "Community 7"
Cohesion: 0.16
Nodes (19): Router, T, TempDir, make_v2_db(), migrates_v2_to_v3_preserving_rows(), receive_first_json_text(), test_api_history_endpoint(), test_api_info_endpoint() (+11 more)

### Community 8 - "Community 8"
Cohesion: 0.05
Nodes (31): Default, Disks, main(), FormatTime, HistoryRepo, deserialize_cpu_data(), deserialize_ram_data(), deserialize_storage_data() (+23 more)

### Community 9 - "Community 9"
Cohesion: 0.26
Nodes (12): Path, Sqlite, history_repo_prune_old_data(), history_repo_save_and_get_recent(), history_repo_save_empty_no_op(), minimal_snapshot(), minimal_system_info(), schema_version_mismatch_purges_tables() (+4 more)

### Community 10 - "Community 10"
Cohesion: 0.14
Nodes (13): extends, internalChecksFilter, lockFileMaintenance, automerge, enabled, rebaseWhen, schedule, minimumReleaseAge (+5 more)

### Community 11 - "Community 11"
Cohesion: 0.33
Nodes (9): ContainerCpuStats, ContainerStatsResponse, process_statistics(), minimal_cpu_stats(), process_statistics_computes_cpu_and_memory(), process_statistics_detects_throttling(), process_statistics_returns_none_when_cpu_stats_missing(), process_statistics_returns_none_when_precpu_stats_missing() (+1 more)

### Community 12 - "Community 12"
Cohesion: 0.40
Nodes (9): history_repo_delete_aggregated_range(), history_repo_delete_raw_range(), history_repo_get_aggregated_snapshots_by_time_range(), history_repo_get_min_raw_created_at_before(), history_repo_get_raw_snapshots_by_time_range(), history_repo_init_creates_aggregated_table(), minimal_aggregated_snapshot(), minimal_snapshot() (+1 more)

### Community 14 - "Community 14"
Cohesion: 0.09
Nodes (20): Changelog v0.8.0, CI Workflow (ci.yml), Docker Build and Push Workflow (docker.yml), Docs Workflow (docs.yml), Release Workflow (release.yml), Tag Version Workflow (tag-version.yml), lib.rs, Rust Agent Rules (.agents/rules/rust.md) (+12 more)

### Community 22 - "Community 22"
Cohesion: 0.09
Nodes (20): Advanced Configuration, `config.toml`, Configuration Files, Custom Port, Database Issues, Directory Structure, `docker-compose.yml`, `.env` (+12 more)

### Community 25 - "Community 25"
Cohesion: 0.09
Nodes (21): [0.1.0] - 2026-02-02, [0.2.0] - 2026-02-04, [0.3.0] - 2026-02-04, [0.4.0] - 2026-02-05, [0.5.0] - 2026-02-06, [0.6.0] - 2026-02-08, [0.6.1] - 2026-02-11, [0.6.2] - 2026-02-11 (+13 more)

### Community 26 - "Community 26"
Cohesion: 0.25
Nodes (7): File Length Limit (300 lines), General Rust Style, Keep `CHANGELOG.md` in Sync, Keep `CODEBASE.md` in Sync, Keep `config.toml` in Sync, Logging Levels, Tests in Separate Files

### Community 27 - "Community 27"
Cohesion: 0.33
Nodes (5): Commit SHAs, Notes, Status: DONE, Summary, Task 2 Report: cliff.toml + CHANGELOG.md

### Community 32 - "Community 32"
Cohesion: 0.15
Nodes (14): AggregatedSnapshot, AppConfig, `AppState`, axum crate, FullSystemSnapshot, history_repo/agg_store.rs, history_repo/aggregation.rs, history_repo/raw.rs (+6 more)

### Community 33 - "Community 33"
Cohesion: 0.33
Nodes (6): CI / CD Workflows, `.github/workflows/ci.yml`, `.github/workflows/docker.yml`, `.github/workflows/docs.yml`, `.github/workflows/release.yml`, `.github/workflows/tag-version.yml`

### Community 34 - "Community 34"
Cohesion: 0.40
Nodes (5): Aggregation Logic (`history_repo::aggregation`), Blob Encoding, History Database (`src/history_repo/`), Key `HistoryRepo` Methods, Tables

### Community 35 - "Community 35"
Cohesion: 0.40
Nodes (5): Aggregation Worker (`src/aggregation_worker.rs`), Backfill (`src/backfill.rs`), History Writer (`src/worker/history_writer.rs`), Main Worker (`src/worker/mod.rs`), Worker Tasks

### Community 36 - "Community 36"
Cohesion: 0.40
Nodes (5): Database Schema, `schema_version`, `system_history`, `system_history_aggregated`, `system_info`

### Community 37 - "Community 37"
Cohesion: 0.67
Nodes (3): Configuration (`src/config.rs`), `[database]` Fields and Defaults, Top-level Sections

## Knowledge Gaps
- **129 isolated node(s):** `docker-entrypoint.sh script`, `$schema`, `extends`, `osvVulnerabilityAlerts`, `minimumReleaseAge` (+124 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **9 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `AppConfig` connect `Community 4` to `Community 1`, `Community 5`, `Community 7`?**
  _High betweenness centrality (0.056) - this node is a cross-community bridge._
- **Why does `FullSystemSnapshot` connect `Community 1` to `Community 0`, `Community 5`, `Community 7`, `Community 8`, `Community 9`, `Community 12`?**
  _High betweenness centrality (0.056) - this node is a cross-community bridge._
- **Why does `Homeserver (Rust)` connect `Community 2` to `Community 32`, `Community 22`?**
  _High betweenness centrality (0.038) - this node is a cross-community bridge._
- **What connects `docker-entrypoint.sh script`, `$schema`, `extends` to the rest of the system?**
  _129 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Community 0` be split into smaller, more focused modules?**
  _Cohesion score 0.07265306122448979 - nodes in this community are weakly interconnected._
- **Should `Community 1` be split into smaller, more focused modules?**
  _Cohesion score 0.09059029807130334 - nodes in this community are weakly interconnected._
- **Should `Community 2` be split into smaller, more focused modules?**
  _Cohesion score 0.056429232192414434 - nodes in this community are weakly interconnected._