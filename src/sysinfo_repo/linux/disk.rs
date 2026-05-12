// Diskstats parsing and `/sys/block` model lookup.

use std::collections::HashMap;

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

pub(crate) fn read_diskstats_linux() -> HashMap<String, DiskIoRaw> {
    #[cfg(target_os = "linux")]
    {
        let content = std::fs::read_to_string("/proc/diskstats").unwrap_or_default();
        parse_diskstats(&content)
    }
    #[cfg(not(target_os = "linux"))]
    HashMap::new()
}

/// Map a block device leaf name (e.g. `sda1`, `nvme0n1p2`) to the parent whole-disk
/// `/sys/block/<name>` node. Returns `name` unchanged when no known partition suffix is present.
pub fn disk_sysfs_base_device_name(name: &str) -> &str {
    let name = name.trim();
    if name.is_empty() {
        return name;
    }

    // NVMe: nvme0n1p2 → nvme0n1
    if name.starts_with("nvme") && name.contains('n') {
        if let Some(i) = name.rfind('p') {
            let tail = name.get(i + 1..).unwrap_or("");
            if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
                return name.get(..i).unwrap_or(name);
            }
        }
        return name;
    }

    // eMMC/MMC: mmcblk0p1 → mmcblk0 (reject mmcblk…boot… false positives vs rfind('p'))
    if let Some(rest) = name.strip_prefix("mmcblk") {
        if let Some(p_pos) = rest.find('p') {
            let left = &rest[..p_pos];
            let right = rest.get(p_pos + 1..).unwrap_or("");
            if left.chars().all(|c| c.is_ascii_digit())
                && right.chars().all(|c| c.is_ascii_digit())
                && !left.is_empty()
                && !right.is_empty()
            {
                let end = "mmcblk".len() + p_pos;
                return name.get(..end).unwrap_or(name);
            }
        }
        return name;
    }

    // sd/vd/xvd + partition digits: sdaa1 → sdaa
    const PREFIXES: [&str; 3] = ["sd", "vd", "xvd"];
    for p in PREFIXES {
        let Some(rest) = name.strip_prefix(p) else {
            continue;
        };
        let Some(di) = rest.find(|c: char| c.is_ascii_digit()) else {
            continue;
        };
        let (letters, digits) = rest.split_at(di);
        if letters.is_empty() || digits.is_empty() {
            continue;
        }
        if digits.chars().all(|c| c.is_ascii_digit()) {
            return name.get(..p.len() + di).unwrap_or(name);
        }
    }

    name
}

/// Read disk model name from `/sys/block/<dev>/device/model` (best-effort).
/// For partitions (`sda1`, …), falls back to the parent block device sysfs node.
pub(crate) fn read_disk_model_linux(dev_name: &str) -> String {
    #[cfg(target_os = "linux")]
    {
        fn read_sysfs_model(node: &str) -> Option<String> {
            let path = format!("/sys/block/{}/device/model", node);
            let ok = std::fs::read_to_string(&path).ok()?;
            let t = ok.trim();
            if t.is_empty() {
                return None;
            }
            Some(t.to_string())
        }

        let n = dev_name.trim();
        read_sysfs_model(n)
            .or_else(|| {
                let base = disk_sysfs_base_device_name(n);
                if base != n {
                    read_sysfs_model(base)
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }
    #[cfg(not(target_os = "linux"))]
    String::new()
}
