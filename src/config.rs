use serde::Deserialize;

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
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
}

fn default_retention_days() -> u32 {
    3
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
            self.database.retention_days > 0,
            "database.retention_days must be > 0, got {}",
            self.database.retention_days
        );
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
