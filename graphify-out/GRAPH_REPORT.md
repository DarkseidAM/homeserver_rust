# Graph Report - .  (2026-06-24)

## Corpus Check
- cluster-only mode — file stats not available

## Summary
- 416 nodes · 816 edges · 25 communities (18 shown, 7 thin omitted)
- Extraction: 96% EXTRACTED · 4% INFERRED · 0% AMBIGUOUS · INFERRED: 29 edges (avg confidence: 0.81)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `3f3f77e1`
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

## God Nodes (most connected - your core abstractions)
1. `FullSystemSnapshot` - 30 edges
2. `DockerRepo` - 16 edges
3. `SystemInfo` - 16 edges
4. `AppState` - 16 edges
5. `AggregatedSnapshot` - 15 edges
6. `ContainerStats` - 15 edges
7. `AppConfig` - 14 edges
8. `SysinfoRepo` - 14 edges
9. `WorkerDeps` - 12 edges
10. `test_app()` - 12 edges

## Surprising Connections (you probably didn't know these)
- `aggregate_snapshots_empty_returns_none()` --calls--> `aggregate_snapshots()`  [INFERRED]
  tests/aggregation_tests.rs → src/history_repo/aggregation.rs
- `aggregate_snapshots_multiple_computes_avg_min_max()` --calls--> `aggregate_snapshots()`  [INFERRED]
  tests/aggregation_tests.rs → src/history_repo/aggregation.rs
- `aggregate_snapshots_single_snapshot()` --calls--> `aggregate_snapshots()`  [INFERRED]
  tests/aggregation_tests.rs → src/history_repo/aggregation.rs
- `snapshot()` --references--> `FullSystemSnapshot`  [EXTRACTED]
  tests/aggregation_tests.rs → src/models/system.rs
- `parse_diskstats_extracts_block_device_counters()` --calls--> `parse_diskstats()`  [INFERRED]
  tests/linux_parser_tests.rs → src/sysinfo_repo/linux/disk.rs

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Core Data Collection and Persistence Pipeline** — codebase_sysinforepo, codebase_dockerrepo, codebase_fullsystemsnapshot, codebase_worker_mod, codebase_worker_history_writer, codebase_historyrepo, codebase_sqlite_system_history [EXTRACTED 1.00]
- **Tiered Aggregation and Downsampling Pipeline** — codebase_sqlite_system_history, codebase_aggregation_worker, codebase_sqlite_system_history_aggregated, codebase_history_repo_aggregation, codebase_history_repo_history_merge, codebase_backfillrs [EXTRACTED 1.00]
- **CI/CD Release Chain (CI → Tag → Docker + Release + Docs)** — ci_ci_yml, ci_tagversion_yml, ci_docker_yml, ci_release_yml, ci_docs_yml [EXTRACTED 1.00]

## Communities (25 total, 7 thin omitted)

### Community 0 - "Community 0"
Cohesion: 0.07
Nodes (30): minimal_snapshot(), main(), HistoryRepo, with_version_prefix(), aggregated_to_snapshot(), deserialize_container_data(), deserialize_network_data(), deserialize_storage_data() (+22 more)

### Community 1 - "Community 1"
Cohesion: 0.10
Nodes (39): Arc, AtomicU64, AtomicUsize, Drop, HistoryRepo, IntoResponse, Query, Receiver (+31 more)

### Community 2 - "Community 2"
Cohesion: 0.07
Nodes (39): Alerting / Health Checks (Missing), Authentication & Authorization (Missing), CPU Temperature (Stubbed Out), Disk I/O Statistics (Stubbed Out), DiskDeviceStat Dead Code Issue, Homeserver-Rust Code Analysis & Netdata Comparison, Load Average Monitoring (Missing), Netdata (+31 more)

### Community 3 - "Community 3"
Cohesion: 0.08
Nodes (38): Changelog v0.8.0, CI Workflow (ci.yml), Docker Build and Push Workflow (docker.yml), Docs Workflow (docs.yml), Release Workflow (release.yml), Tag Version Workflow (tag-version.yml), AggregatedSnapshot, aggregation_worker.rs (+30 more)

### Community 4 - "Community 4"
Cohesion: 0.06
Nodes (5): AppConfig, MonitoringConfig, normalize_cron_expression(), PublishingConfig, ServerConfig

### Community 5 - "Community 5"
Cohesion: 0.09
Nodes (24): HashMap, disk_sysfs_base_device_name(), DiskIoRaw, parse_diskstats(), read_disk_model_linux(), read_diskstats_linux(), parse_hwmon_temp(), parse_loadavg() (+16 more)

### Community 6 - "Community 6"
Cohesion: 0.10
Nodes (22): Docker, DockerRepo, aggregate_aggregated_snapshots(), aggregate_containers(), aggregate_containers_from_aggregated(), aggregate_one_container(), aggregate_snapshots(), init_aggregated_table() (+14 more)

### Community 7 - "Community 7"
Cohesion: 0.20
Nodes (17): Router, T, TempDir, receive_first_json_text(), test_api_history_endpoint(), test_api_info_endpoint(), test_app(), test_app_config() (+9 more)

### Community 8 - "Community 8"
Cohesion: 0.14
Nodes (8): Default, Disks, Instant, Mutex, Networks, Self, SysinfoRepo, System

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
Cohesion: 0.38
Nodes (5): FormatTime, LocalTimer, main(), shutdown_signal(), Writer

## Knowledge Gaps
- **40 isolated node(s):** `docker-entrypoint.sh script`, `$schema`, `extends`, `osvVulnerabilityAlerts`, `minimumReleaseAge` (+35 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **7 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `FullSystemSnapshot` connect `Community 0` to `Community 1`, `Community 6`, `Community 7`, `Community 9`, `Community 12`?**
  _High betweenness centrality (0.097) - this node is a cross-community bridge._
- **Why does `AppConfig` connect `Community 4` to `Community 1`, `Community 5`, `Community 7`?**
  _High betweenness centrality (0.097) - this node is a cross-community bridge._
- **Why does `AppState` connect `Community 1` to `Community 0`, `Community 4`?**
  _High betweenness centrality (0.044) - this node is a cross-community bridge._
- **What connects `docker-entrypoint.sh script`, `$schema`, `extends` to the rest of the system?**
  _40 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Community 0` be split into smaller, more focused modules?**
  _Cohesion score 0.07033315705975675 - nodes in this community are weakly interconnected._
- **Should `Community 1` be split into smaller, more focused modules?**
  _Cohesion score 0.10431372549019607 - nodes in this community are weakly interconnected._
- **Should `Community 2` be split into smaller, more focused modules?**
  _Cohesion score 0.07152496626180836 - nodes in this community are weakly interconnected._