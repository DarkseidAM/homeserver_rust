// Build-time version from Cargo.toml

/// Package version (from Cargo.toml).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Package name (from Cargo.toml).
pub const NAME: &str = env!("CARGO_PKG_NAME");
