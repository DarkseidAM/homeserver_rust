// Linux-specific helpers: /proc, /etc/os-release, DMI, interface speed.

mod disk;

pub use disk::{DiskIoRaw, disk_sysfs_base_device_name, parse_diskstats};
pub(crate) use disk::{read_disk_model_linux, read_diskstats_linux};

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
