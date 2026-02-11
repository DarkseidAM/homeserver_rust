use serde::Deserialize;
use std::str::FromStr;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub publishing: PublishingConfig,
    pub monitoring: MonitoringConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub host: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub path: String,
    pub max_pool_size: u32,
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
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let path = std::env::var("CONFIG_FILE").unwrap_or_else(|_| "config.toml".into());
        let s = std::fs::read_to_string(&path)?;
        Self::load_from_str(&s)
    }

    /// Parse and validate config from a string (e.g. for tests).
    pub fn load_from_str(s: &str) -> anyhow::Result<Self> {
        let config: AppConfig = toml::from_str(s)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.server.port > 0,
            "server.port must be between 1 and 65535, got {}",
            self.server.port
        );
        anyhow::ensure!(
            !self.database.path.is_empty(),
            "database.path must be non-empty"
        );
        anyhow::ensure!(
            self.database.max_pool_size > 0,
            "database.max_pool_size must be > 0, got {}",
            self.database.max_pool_size
        );
        anyhow::ensure!(
            self.database.flush_rate > 0,
            "database.flush_rate must be > 0, got {}",
            self.database.flush_rate
        );
        anyhow::ensure!(
            self.database.flush_interval_secs > 0,
            "database.flush_interval_secs must be > 0, got {}",
            self.database.flush_interval_secs
        );
        anyhow::ensure!(
            self.database.retention_days > 0,
            "database.retention_days must be > 0, got {}",
            self.database.retention_days
        );
        anyhow::ensure!(
            self.database.prune_interval_secs > 0,
            "database.prune_interval_secs must be > 0, got {}",
            self.database.prune_interval_secs
        );
        if let Some(ref cron_str) = self.database.vacuum_schedule {
            cron::Schedule::from_str(cron_str).map_err(|e| {
                anyhow::anyhow!("database.vacuum_schedule invalid cron expression: {}", e)
            })?;
        } else {
            anyhow::ensure!(
                self.database.vacuum_interval_secs > 0,
                "database.vacuum_interval_secs must be > 0 when vacuum_schedule is not set, got {}",
                self.database.vacuum_interval_secs
            );
        }
        if self.database.enable_aggregation {
            anyhow::ensure!(
                self.database.aggregation_interval_secs > 0,
                "database.aggregation_interval_secs must be > 0 when enable_aggregation is true, got {}",
                self.database.aggregation_interval_secs
            );
            anyhow::ensure!(
                self.database.raw_retention_hours > 0,
                "database.raw_retention_hours must be > 0 when enable_aggregation is true, got {}",
                self.database.raw_retention_hours
            );
            anyhow::ensure!(
                self.database.minute_retention_hours > 0,
                "database.minute_retention_hours must be > 0 when enable_aggregation is true, got {}",
                self.database.minute_retention_hours
            );
        }
        anyhow::ensure!(
            self.publishing.cpu_stats_frequency_ms > 0,
            "publishing.cpu_stats_frequency_ms must be > 0, got {}",
            self.publishing.cpu_stats_frequency_ms
        );
        anyhow::ensure!(
            self.publishing.ram_stats_frequency_ms > 0,
            "publishing.ram_stats_frequency_ms must be > 0, got {}",
            self.publishing.ram_stats_frequency_ms
        );
        anyhow::ensure!(
            self.publishing.broadcast_capacity > 0,
            "publishing.broadcast_capacity must be > 0, got {}",
            self.publishing.broadcast_capacity
        );
        anyhow::ensure!(
            self.monitoring.sample_interval_ms > 0,
            "monitoring.sample_interval_ms must be > 0, got {}",
            self.monitoring.sample_interval_ms
        );
        anyhow::ensure!(
            self.monitoring.stats_log_interval_secs > 0,
            "monitoring.stats_log_interval_secs must be > 0, got {}",
            self.monitoring.stats_log_interval_secs
        );
        Ok(())
    }
}
