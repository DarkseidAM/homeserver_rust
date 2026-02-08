// Config loading and validation tests

use homeserver::config::AppConfig;

const VALID_CONFIG: &str = r#"
[server]
port = 8081
host = "0.0.0.0"

[database]
path = "data/server.db"
max_pool_size = 10
flush_rate = 10

[publishing]
cpu_stats_frequency_ms = 1000
ram_stats_frequency_ms = 1000
broadcast_capacity = 60

[monitoring]
sample_interval_ms = 1000
stats_log_interval_secs = 60
"#;

#[test]
fn test_config_loads_from_str() {
    let config = AppConfig::load_from_str(VALID_CONFIG).expect("load_from_str");
    assert_eq!(config.server.port, 8081);
    assert_eq!(config.server.host, "0.0.0.0");
    assert_eq!(config.database.path, "data/server.db");
    assert_eq!(config.database.flush_rate, 10);
    assert_eq!(config.publishing.broadcast_capacity, 60);
    assert_eq!(config.monitoring.sample_interval_ms, 1000);
}

#[test]
fn test_config_validation_rejects_invalid_port() {
    let bad = VALID_CONFIG.replace("port = 8081", "port = 0");
    let err = AppConfig::load_from_str(&bad).unwrap_err();
    assert!(err.to_string().contains("server.port"));
}

#[test]
fn test_config_validation_rejects_empty_db_path() {
    let bad = VALID_CONFIG.replace("path = \"data/server.db\"", "path = \"\"");
    let err = AppConfig::load_from_str(&bad).unwrap_err();
    assert!(err.to_string().contains("database.path"));
}

#[test]
fn test_config_validation_rejects_max_pool_size_zero() {
    let bad = VALID_CONFIG.replace("max_pool_size = 10", "max_pool_size = 0");
    let err = AppConfig::load_from_str(&bad).unwrap_err();
    assert!(err.to_string().contains("max_pool_size"));
}

#[test]
fn test_config_validation_rejects_flush_rate_zero() {
    let bad = VALID_CONFIG.replace("flush_rate = 10", "flush_rate = 0");
    let err = AppConfig::load_from_str(&bad).unwrap_err();
    assert!(err.to_string().contains("flush_rate"));
}

#[test]
fn test_config_validation_rejects_cpu_stats_frequency_zero() {
    let bad = VALID_CONFIG.replace(
        "cpu_stats_frequency_ms = 1000",
        "cpu_stats_frequency_ms = 0",
    );
    let err = AppConfig::load_from_str(&bad).unwrap_err();
    assert!(err.to_string().contains("cpu_stats_frequency_ms"));
}

#[test]
fn test_config_validation_rejects_ram_stats_frequency_zero() {
    let bad = VALID_CONFIG.replace(
        "ram_stats_frequency_ms = 1000",
        "ram_stats_frequency_ms = 0",
    );
    let err = AppConfig::load_from_str(&bad).unwrap_err();
    assert!(err.to_string().contains("ram_stats_frequency_ms"));
}

#[test]
fn test_config_validation_rejects_broadcast_capacity_zero() {
    let bad = VALID_CONFIG.replace("broadcast_capacity = 60", "broadcast_capacity = 0");
    let err = AppConfig::load_from_str(&bad).unwrap_err();
    assert!(err.to_string().contains("broadcast_capacity"));
}

#[test]
fn test_config_validation_rejects_sample_interval_zero() {
    let bad = VALID_CONFIG.replace("sample_interval_ms = 1000", "sample_interval_ms = 0");
    let err = AppConfig::load_from_str(&bad).unwrap_err();
    assert!(err.to_string().contains("sample_interval_ms"));
}

#[test]
fn test_config_validation_rejects_stats_log_interval_zero() {
    let bad = VALID_CONFIG.replace(
        "stats_log_interval_secs = 60",
        "stats_log_interval_secs = 0",
    );
    let err = AppConfig::load_from_str(&bad).unwrap_err();
    assert!(err.to_string().contains("stats_log_interval_secs"));
}

#[test]
fn test_config_validation_rejects_invalid_toml() {
    let err = AppConfig::load_from_str("not valid toml [[[").unwrap_err();
    assert!(!err.to_string().is_empty());
}

#[test]
fn test_config_load_from_file_via_env() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, VALID_CONFIG).unwrap();
    unsafe { std::env::set_var("CONFIG_FILE", path.to_str().unwrap()) };
    let result = AppConfig::load();
    unsafe { std::env::remove_var("CONFIG_FILE") };
    let config = result.expect("load from CONFIG_FILE");
    assert_eq!(config.server.port, 8081);
    assert_eq!(config.database.path, "data/server.db");
}

#[test]
fn test_config_aggregation_defaults_when_omitted() {
    let config = AppConfig::load_from_str(VALID_CONFIG).expect("valid");
    assert!(config.database.enable_aggregation);
    assert_eq!(config.database.aggregation_interval_secs, 3600);
    assert_eq!(config.database.raw_retention_hours, 1);
    assert_eq!(config.database.minute_retention_hours, 24);
}

const VALID_CONFIG_WITH_AGGREGATION: &str = r#"
[server]
port = 8081
host = "0.0.0.0"

[database]
path = "data/server.db"
max_pool_size = 10
flush_rate = 10
retention_days = 3
enable_aggregation = true
aggregation_interval_secs = 3600
raw_retention_hours = 1
minute_retention_hours = 24

[publishing]
cpu_stats_frequency_ms = 1000
ram_stats_frequency_ms = 1000
broadcast_capacity = 60

[monitoring]
sample_interval_ms = 1000
stats_log_interval_secs = 60
"#;

#[test]
fn test_config_loads_with_aggregation() {
    let config = AppConfig::load_from_str(VALID_CONFIG_WITH_AGGREGATION).expect("valid");
    assert!(config.database.enable_aggregation);
    assert_eq!(config.database.aggregation_interval_secs, 3600);
    assert_eq!(config.database.raw_retention_hours, 1);
    assert_eq!(config.database.minute_retention_hours, 24);
}

#[test]
fn test_config_validation_rejects_aggregation_interval_zero_when_enabled() {
    let bad = VALID_CONFIG_WITH_AGGREGATION.replace(
        "aggregation_interval_secs = 3600",
        "aggregation_interval_secs = 0",
    );
    let err = AppConfig::load_from_str(&bad).unwrap_err();
    assert!(err.to_string().contains("aggregation_interval_secs"));
}

#[test]
fn test_config_validation_rejects_raw_retention_hours_zero_when_enabled() {
    let bad =
        VALID_CONFIG_WITH_AGGREGATION.replace("raw_retention_hours = 1", "raw_retention_hours = 0");
    let err = AppConfig::load_from_str(&bad).unwrap_err();
    assert!(err.to_string().contains("raw_retention_hours"));
}

#[test]
fn test_config_validation_rejects_minute_retention_hours_zero_when_enabled() {
    let bad = VALID_CONFIG_WITH_AGGREGATION
        .replace("minute_retention_hours = 24", "minute_retention_hours = 0");
    let err = AppConfig::load_from_str(&bad).unwrap_err();
    assert!(err.to_string().contains("minute_retention_hours"));
}
