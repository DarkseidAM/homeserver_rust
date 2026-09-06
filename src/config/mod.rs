mod models;

pub use models::*;

use std::str::FromStr;

/// Converts 5-field Unix cron (min hour dom month dow) to 6-field (sec min hour dom month dow)
/// by prepending "0" for seconds. The cron crate expects at least 6 fields.
pub(crate) fn normalize_cron_expression(s: &str) -> String {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() == 5 {
        format!("0 {}", parts.join(" "))
    } else {
        s.trim().to_string()
    }
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
            let normalized = normalize_cron_expression(cron_str);
            cron::Schedule::from_str(&normalized).map_err(|e| {
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
        for rule in &self.alerts.rules {
            anyhow::ensure!(!rule.name.is_empty(), "alert rule name must be non-empty");
            anyhow::ensure!(
                ALERT_METRICS.contains(&rule.metric.as_str()),
                "alert rule '{}' has unknown metric '{}' (expected one of {:?})",
                rule.name,
                rule.metric,
                ALERT_METRICS
            );
            anyhow::ensure!(
                matches!(rule.op.as_str(), ">" | ">=" | "<" | "<="),
                "alert rule '{}' has invalid op '{}' (expected >, >=, <, <=)",
                rule.name,
                rule.op
            );
        }
        Ok(())
    }
}
