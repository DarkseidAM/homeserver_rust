// SMART disk-health collector. Shells out to `smartctl --json` on a slow cadence (it is
// expensive and privileged) and caches the result; the worker reads the cache each tick.

mod parse;

pub use parse::{parse_scan_devices, parse_smartctl_json};

use crate::models::SmartHealth;
use std::sync::{Arc, Mutex};
use tokio::process::Command;

/// Holds the most recent SMART readings, refreshed by a slow background poll.
pub struct SmartRepo {
    cache: Arc<Mutex<Vec<SmartHealth>>>,
}

impl SmartRepo {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Current cached SMART readings (empty until the first successful refresh).
    pub fn current(&self) -> Vec<SmartHealth> {
        self.cache.lock().map(|g| g.clone()).unwrap_or_default()
    }

    /// Re-scan devices and refresh the cache. Tolerates a missing/failing `smartctl`
    /// (logs once at warn and leaves the cache unchanged).
    pub async fn refresh(&self) {
        let scan = match run_smartctl(&["--scan", "-j"]).await {
            Some(out) => out,
            None => {
                tracing::warn!(
                    operation = "smart_scan",
                    "smartctl unavailable or failed; SMART data not collected (need smartmontools + device privileges)"
                );
                return;
            }
        };
        let devices = parse_scan_devices(&scan);
        let mut out = Vec::with_capacity(devices.len());
        for device in devices {
            if let Some(json) = run_smartctl(&["-a", "-j", &device]).await
                && let Some(health) = parse_smartctl_json(&json, &device)
            {
                out.push(health);
            }
        }
        if let Ok(mut guard) = self.cache.lock() {
            *guard = out;
        }
    }
}

impl Default for SmartRepo {
    fn default() -> Self {
        Self::new()
    }
}

/// Run `smartctl <args>` and return stdout. smartctl uses non-zero exit codes to encode
/// disk warnings (bitmask), so stdout is parsed regardless of exit status as long as it ran.
async fn run_smartctl(args: &[&str]) -> Option<String> {
    let output = Command::new("smartctl").args(args).output().await.ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if stdout.trim().is_empty() {
        return None;
    }
    Some(stdout)
}
