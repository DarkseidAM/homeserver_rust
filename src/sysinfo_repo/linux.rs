// Linux-specific helpers: /proc, /etc/os-release, DMI, interface speed.

use std::collections::HashMap;

// ── Load average ─────────────────────────────────────────────────────────────

/// Parse `/proc/loadavg` content into (1min, 5min, 15min) averages.
pub fn parse_loadavg(content: &str) -> Option<(f64, f64, f64)> {
    let mut parts = content.split_whitespace();
    let one = parts.next()?.parse::<f64>().ok()?;
    let five = parts.next()?.parse::<f64>().ok()?;
    let fifteen = parts.next()?.parse::<f64>().ok()?;
    Some((one, five, fifteen))
}

pub(super) fn read_loadavg_linux() -> Option<(f64, f64, f64)> {
    #[cfg(target_os = "linux")]
    {
        let content = std::fs::read_to_string("/proc/loadavg").ok()?;
        parse_loadavg(&content)
    }
    #[cfg(not(target_os = "linux"))]
    None
}

// ── CPU temperature ──────────────────────────────────────────────────────────

/// Parse a sysfs temperature file (millidegrees Celsius) into degrees.
pub fn parse_hwmon_temp(content: &str) -> Option<f64> {
    let millideg: i64 = content.trim().parse().ok()?;
    Some(millideg as f64 / 1000.0)
}

pub(super) fn read_cpu_temperature_linux() -> Option<f64> {
    #[cfg(target_os = "linux")]
    {
        // Prefer coretemp / k10temp hwmon sensors
        for i in 0..16 {
            let name_path = format!("/sys/class/hwmon/hwmon{}/name", i);
            let temp_path = format!("/sys/class/hwmon/hwmon{}/temp1_input", i);
            if let Ok(name) = std::fs::read_to_string(&name_path) {
                let name = name.trim();
                if matches!(name, "coretemp" | "k10temp" | "zenpower")
                    && let Ok(c) = std::fs::read_to_string(&temp_path)
                    && let Some(t) = parse_hwmon_temp(&c)
                {
                    return Some(t);
                }
            }
        }
        // Fall back: any hwmon temp1_input
        for i in 0..16 {
            let path = format!("/sys/class/hwmon/hwmon{}/temp1_input", i);
            if let Ok(c) = std::fs::read_to_string(&path)
                && let Some(t) = parse_hwmon_temp(&c)
            {
                return Some(t);
            }
        }
        // Fall back: thermal_zone (ARM/container)
        for i in 0..8 {
            let path = format!("/sys/class/thermal/thermal_zone{}/temp", i);
            if let Ok(c) = std::fs::read_to_string(&path)
                && let Some(t) = parse_hwmon_temp(&c)
            {
                return Some(t);
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    None
}

// ── Disk I/O stats ───────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone)]
pub struct DiskIoRaw {
    pub reads_completed: u64,
    pub sectors_read: u64,
    pub writes_completed: u64,
    pub sectors_written: u64,
    pub io_time_ms: u64,
}

/// Parse `/proc/diskstats` content into a map of device name → I/O counters.
/// Skips loop, ram, and zram virtual devices.
pub fn parse_diskstats(content: &str) -> HashMap<String, DiskIoRaw> {
    #[inline]
    fn u64_field(it: &mut std::str::SplitWhitespace<'_>) -> Option<u64> {
        Some(it.next()?.parse::<u64>().unwrap_or(0))
    }

    #[inline]
    fn skip_field(it: &mut std::str::SplitWhitespace<'_>) -> Option<()> {
        it.next()?;
        Some(())
    }

    // One diskstats row: major, minor, device name, then kernel counter fields through io_time_ms.
    fn diskstats_row(line: &str) -> Option<(String, DiskIoRaw)> {
        let mut it = line.split_whitespace();
        skip_field(&mut it)?;
        skip_field(&mut it)?;
        let name = it.next()?;
        if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("zram") {
            return None;
        }

        let reads_completed = u64_field(&mut it)?;
        skip_field(&mut it)?; // reads merged
        let sectors_read = u64_field(&mut it)?;
        skip_field(&mut it)?; // time reading (ms)
        let writes_completed = u64_field(&mut it)?;
        skip_field(&mut it)?; // writes merged
        let sectors_written = u64_field(&mut it)?;
        skip_field(&mut it)?; // time writing (ms)
        skip_field(&mut it)?; // I/Os in progress
        let io_time_ms = u64_field(&mut it)?;

        Some((
            name.to_string(),
            DiskIoRaw {
                reads_completed,
                sectors_read,
                writes_completed,
                sectors_written,
                io_time_ms,
            },
        ))
    }

    let mut map = HashMap::new();
    for line in content.lines() {
        if let Some((name, raw)) = diskstats_row(line) {
            map.insert(name, raw);
        }
    }
    map
}

pub(super) fn read_diskstats_linux() -> HashMap<String, DiskIoRaw> {
    #[cfg(target_os = "linux")]
    {
        let content = std::fs::read_to_string("/proc/diskstats").unwrap_or_default();
        parse_diskstats(&content)
    }
    #[cfg(not(target_os = "linux"))]
    HashMap::new()
}

/// Read disk model name from `/sys/block/<dev>/device/model` (best-effort).
pub(super) fn read_disk_model_linux(dev_name: &str) -> String {
    #[cfg(target_os = "linux")]
    {
        let path = format!("/sys/block/{}/device/model", dev_name);
        std::fs::read_to_string(&path)
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    }
    #[cfg(not(target_os = "linux"))]
    String::new()
}

// ── Network interface operstate ───────────────────────────────────────────────

/// Parse the content of `/sys/class/net/<iface>/operstate`.
/// Returns `true` for "up" or "unknown" (virtual/loopback), false for "down" etc.
pub fn parse_operstate(content: &str) -> bool {
    matches!(content.trim(), "up" | "unknown")
}

pub(super) fn read_interface_operstate(name: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        let path = format!("/sys/class/net/{}/operstate", name);
        std::fs::read_to_string(&path)
            .map(|c| parse_operstate(&c))
            .unwrap_or(true) // unreadable → assume up (preserve old behaviour)
    }
    #[cfg(not(target_os = "linux"))]
    true
}

/// Read first "model name" from /proc/cpuinfo (Linux). Prefer over sysinfo when it returns "cpu0" etc.
pub(super) fn read_cpu_model_linux() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let content = std::fs::read_to_string("/proc/cpuinfo").ok()?;
        for line in content.lines() {
            if line.starts_with("model name") {
                let name = line
                    .find(": ")
                    .map(|i| line[i + 2..].trim())
                    .filter(|s| !s.is_empty() && *s != "cpu0")?;
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Read OS/distro name from /etc/os-release (Linux) for os_manufacturer.
pub(super) fn read_os_manufacturer_linux() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let content = std::fs::read_to_string("/etc/os-release").ok()?;
        for line in content.lines() {
            if line.starts_with("PRETTY_NAME=") {
                let v = line.strip_prefix("PRETTY_NAME=")?.trim_matches('"');
                return if v.is_empty() {
                    None
                } else {
                    Some(v.to_string())
                };
            }
        }
        for line in content.lines() {
            if line.starts_with("NAME=") {
                let v = line.strip_prefix("NAME=")?.trim_matches('"');
                return if v.is_empty() {
                    None
                } else {
                    Some(v.to_string())
                };
            }
        }
    }
    None
}

/// Read system vendor from DMI (Linux).
pub(super) fn read_sys_vendor_linux() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let v = std::fs::read_to_string("/sys/class/dmi/id/sys_vendor").ok()?;
        let v = v.trim();
        if v.is_empty() {
            return None;
        }
        Some(v.to_string())
    }
    #[cfg(not(target_os = "linux"))]
    None
}

/// Read network interface link speed from /sys/class/net/<interface>/speed (Linux).
/// Returns speed in bits per second (like OSHI), or 0 if unavailable.
pub(super) fn get_interface_speed(interface_name: &str) -> u64 {
    #[cfg(target_os = "linux")]
    {
        let path = format!("/sys/class/net/{}/speed", interface_name);
        if let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(mbps) = content.trim().parse::<i64>()
            && mbps > 0
        {
            return (mbps as u64) * 1_000_000;
        }
    }
    0
}
