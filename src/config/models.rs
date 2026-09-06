use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub publishing: PublishingConfig,
    pub monitoring: MonitoringConfig,
    #[serde(default)]
    pub alerts: AlertsConfig,
}

/// Threshold-based alerting. `webhook_url` (optional) receives a JSON POST per event;
/// every event is also logged via `tracing`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AlertsConfig {
    #[serde(default)]
    pub webhook_url: Option<String>,
    #[serde(default)]
    pub rules: Vec<AlertRule>,
}

/// One alert rule: fire when `metric op threshold` holds for `duration_secs`, then debounce
/// re-notification for `cooldown_secs`.
#[derive(Debug, Clone, Deserialize)]
pub struct AlertRule {
    pub name: String,
    /// One of: cpu_usage, mem_usage_percent, swap_usage_percent, load_avg_1, cpu_temperature,
    /// disk_usage_percent, gpu_temperature, gpu_utilization.
    pub metric: String,
    /// Comparison operator: ">", ">=", "<", "<=".
    pub op: String,
    pub threshold: f64,
    #[serde(default)]
    pub duration_secs: u64,
    #[serde(default = "default_cooldown_secs")]
    pub cooldown_secs: u64,
}

fn default_cooldown_secs() -> u64 {
    300
}

/// Metric names accepted in alert rules.
pub(crate) const ALERT_METRICS: &[&str] = &[
    "cpu_usage",
    "mem_usage_percent",
    "swap_usage_percent",
    "load_avg_1",
    "cpu_temperature",
    "disk_usage_percent",
    "gpu_temperature",
    "gpu_utilization",
];

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub host: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub path: String,
    pub max_pool_size: u32,
    /// Memory-mapped I/O size limit in MB (SQLite mmap_size pragma). Defaults to 128 MB.
    #[serde(default = "default_mmap_size_mb")]
    pub mmap_size_mb: u64,
    pub flush_rate: u64,
    /// Flush at least every N seconds even if buffer below flush_rate (writer task).
    #[serde(default = "default_flush_interval_secs")]
    pub flush_interval_secs: u64,
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    /// How often to prune old raw data (seconds). Independent of sample_interval_ms.
    #[serde(default = "default_prune_interval_secs")]
    pub prune_interval_secs: u64,
    #[serde(default = "default_enable_aggregation")]
    pub enable_aggregation: bool,
    #[serde(default = "default_aggregation_interval_secs")]
    pub aggregation_interval_secs: u64,
    #[serde(default = "default_raw_retention_hours")]
    pub raw_retention_hours: u32,
    #[serde(default = "default_minute_retention_hours")]
    pub minute_retention_hours: u32,
    /// Optional cron expression for VACUUM (e.g. "0 3 * * *" = 03:00 daily). Uses local time.
    #[serde(default)]
    pub vacuum_schedule: Option<String>,
    /// Fallback: run VACUUM every N seconds when vacuum_schedule is not set. Default 86400 (24h).
    #[serde(default = "default_vacuum_interval_secs")]
    pub vacuum_interval_secs: u64,
    /// Persist GPU metrics to history (gpu_data blobs). Live WS always includes GPUs regardless.
    #[serde(default = "default_true")]
    pub persist_gpu: bool,
    /// Persist SMART disk health to history (smart_data blobs). Live WS always includes it regardless.
    #[serde(default = "default_true")]
    pub persist_smart: bool,
}

fn default_mmap_size_mb() -> u64 {
    128
}

fn default_true() -> bool {
    true
}

fn default_smart_poll_interval_secs() -> u64 {
    900
}

fn default_retention_days() -> u32 {
    3
}

fn default_flush_interval_secs() -> u64 {
    30
}

fn default_prune_interval_secs() -> u64 {
    3600
}

fn default_vacuum_interval_secs() -> u64 {
    86400
}

fn default_enable_aggregation() -> bool {
    true
}

fn default_aggregation_interval_secs() -> u64 {
    3600
}

fn default_raw_retention_hours() -> u32 {
    1
}

fn default_minute_retention_hours() -> u32 {
    24
}

#[derive(Debug, Clone, Deserialize)]
pub struct PublishingConfig {
    pub cpu_stats_frequency_ms: u64,
    pub ram_stats_frequency_ms: u64,
    /// Max number of full-system snapshots kept in the broadcast channel for /ws/system (slow clients may lag).
    pub broadcast_capacity: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MonitoringConfig {
    pub sample_interval_ms: u64,
    /// How often to log app stats (ws_system clients, snapshots saved/pruned) at INFO level.
    pub stats_log_interval_secs: u64,
    /// Collect GPU metrics each tick (NVIDIA needs the `gpu-nvidia` build feature; AMD/Intel via /sys).
    #[serde(default = "default_true")]
    pub collect_gpu: bool,
    /// Collect SMART disk health (requires smartctl + device privileges). Off by default.
    #[serde(default)]
    pub collect_smart: bool,
    /// How often to refresh SMART data (seconds). SMART reads are slow/privileged.
    #[serde(default = "default_smart_poll_interval_secs")]
    pub smart_poll_interval_secs: u64,
}
