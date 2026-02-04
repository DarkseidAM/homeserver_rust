// Linux-specific helpers: /proc, /etc/os-release, DMI, interface speed.

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
