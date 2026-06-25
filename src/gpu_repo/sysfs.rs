// AMD + Intel GPU stats via /sys/class/drm/card<N>/device (+ hwmon).
// NVIDIA is handled by the NVML backend (feature `gpu-nvidia`) and skipped here.
//
// Pure parse_* helpers are split from file I/O so they are unit-testable without hardware.

use crate::models::GpuStats;

/// Map a PCI vendor id (e.g. "0x1002") to our vendor label. NVIDIA is intentionally excluded
/// (covered by NVML). Returns None for unknown/NVIDIA vendors.
pub fn parse_vendor_id(content: &str) -> Option<&'static str> {
    match content.trim().to_ascii_lowercase().as_str() {
        "0x1002" => Some("amd"),
        "0x8086" => Some("intel"),
        _ => None,
    }
}

/// Parse an integer percentage file (e.g. `gpu_busy_percent`), clamped to 0..=100.
pub fn parse_busy_percent(content: &str) -> Option<f64> {
    content
        .trim()
        .parse::<f64>()
        .ok()
        .map(|v| v.clamp(0.0, 100.0))
}

/// Parse an unsigned integer file (e.g. `mem_info_vram_used` in bytes).
pub fn parse_u64(content: &str) -> Option<u64> {
    content.trim().parse::<u64>().ok()
}

/// hwmon `power*_average` is in microwatts; convert to watts.
pub fn parse_power_microwatts(content: &str) -> Option<f64> {
    content.trim().parse::<u64>().ok().map(|uw| uw as f64 / 1e6)
}

/// hwmon `pwm1` fan duty is 0..=255; convert to a percentage.
pub fn parse_pwm_percent(content: &str) -> Option<f64> {
    content
        .trim()
        .parse::<u32>()
        .ok()
        .map(|v| (v as f64 / 255.0 * 100.0).clamp(0.0, 100.0))
}

/// Collect AMD/Intel GPUs from sysfs. Never errors; unreadable fields default to 0/None.
pub fn collect() -> Vec<GpuStats> {
    #[cfg(target_os = "linux")]
    {
        collect_linux()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Vec::new()
    }
}

#[cfg(target_os = "linux")]
fn collect_linux() -> Vec<GpuStats> {
    use std::fs::read_to_string;

    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
        return out;
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        // Match "card0", "card1"… but not render nodes ("card0-DP-1") or "renderD*".
        if !is_card_node(name) {
            continue;
        }
        let dev = entry.path().join("device");
        let Some(vendor) = read_to_string(dev.join("vendor"))
            .ok()
            .and_then(|c| parse_vendor_id(&c))
        else {
            continue; // unknown vendor or NVIDIA (handled by NVML)
        };

        let index = name
            .strip_prefix("card")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        let utilization_percent = read_to_string(dev.join("gpu_busy_percent"))
            .ok()
            .and_then(|c| parse_busy_percent(&c))
            .unwrap_or(0.0);
        let memory_used_bytes = read_to_string(dev.join("mem_info_vram_used"))
            .ok()
            .and_then(|c| parse_u64(&c))
            .unwrap_or(0);
        let memory_total_bytes = read_to_string(dev.join("mem_info_vram_total"))
            .ok()
            .and_then(|c| parse_u64(&c))
            .unwrap_or(0);

        let (temperature_c, power_watts, fan_percent) = read_hwmon(&dev);

        out.push(GpuStats {
            index,
            vendor: vendor.to_string(),
            name: format!("{} GPU {}", vendor, index),
            utilization_percent,
            memory_used_bytes,
            memory_total_bytes,
            temperature_c,
            power_watts,
            fan_percent,
        });
    }
    // sort_by (not sort_by_key) to compare vendor by reference — avoids cloning the String per comparison.
    out.sort_by(|a, b| (&a.vendor, a.index).cmp(&(&b.vendor, b.index)));
    out
}

/// True for whole-card nodes like "card0", not render subnodes or "renderD128".
fn is_card_node(name: &str) -> bool {
    name.strip_prefix("card")
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
}

/// Read temperature (°C), power (W), and fan (%) from the device's first hwmon directory.
#[cfg(target_os = "linux")]
fn read_hwmon(dev: &std::path::Path) -> (f64, Option<f64>, Option<f64>) {
    use crate::sysinfo_repo::linux::parse_hwmon_temp;
    use std::fs::read_to_string;

    // GPU hwmon data lives under the first hwmon* subdirectory of the device.
    let first_hwmon = std::fs::read_dir(dev.join("hwmon"))
        .ok()
        .and_then(|entries| entries.flatten().next());
    let Some(entry) = first_hwmon else {
        return (0.0, None, None);
    };
    let base = entry.path();
    let temperature_c = read_to_string(base.join("temp1_input"))
        .ok()
        .and_then(|c| parse_hwmon_temp(&c))
        .unwrap_or(0.0);
    let power_watts = read_to_string(base.join("power1_average"))
        .ok()
        .and_then(|c| parse_power_microwatts(&c));
    let fan_percent = read_to_string(base.join("pwm1"))
        .ok()
        .and_then(|c| parse_pwm_percent(&c));
    (temperature_c, power_watts, fan_percent)
}
