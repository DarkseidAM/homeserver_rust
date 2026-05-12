// Unit tests for pure Linux /proc and /sys parser functions.
// These functions have no I/O side effects; all assertions are over string literals.

use homeserver::sysinfo_repo::linux::{
    disk_sysfs_base_device_name, parse_diskstats, parse_hwmon_temp, parse_loadavg, parse_operstate,
};

// ── parse_loadavg ─────────────────────────────────────────────────────────────

#[test]
fn parse_loadavg_extracts_three_values() {
    let (one, five, fifteen) = parse_loadavg("0.52 1.23 2.34 1/234 5678").unwrap();
    assert!((one - 0.52).abs() < 0.001);
    assert!((five - 1.23).abs() < 0.001);
    assert!((fifteen - 2.34).abs() < 0.001);
}

#[test]
fn parse_loadavg_returns_none_for_invalid_content() {
    assert!(parse_loadavg("not valid").is_none());
    assert!(parse_loadavg("").is_none());
}

#[test]
fn parse_loadavg_handles_high_load() {
    let (one, _, _) = parse_loadavg("32.00 16.50 8.25 4/100 99999").unwrap();
    assert!((one - 32.0).abs() < 0.001);
}

// ── parse_hwmon_temp ──────────────────────────────────────────────────────────

#[test]
fn parse_hwmon_temp_converts_millidegrees() {
    assert!((parse_hwmon_temp("45000\n").unwrap() - 45.0).abs() < 0.001);
    assert!((parse_hwmon_temp("72500").unwrap() - 72.5).abs() < 0.001);
    assert!((parse_hwmon_temp("0").unwrap() - 0.0).abs() < 0.001);
}

#[test]
fn parse_hwmon_temp_returns_none_for_invalid() {
    assert!(parse_hwmon_temp("invalid").is_none());
    assert!(parse_hwmon_temp("").is_none());
    assert!(parse_hwmon_temp("45.0").is_none()); // floats not accepted here
}

// ── parse_diskstats ───────────────────────────────────────────────────────────

#[test]
fn parse_diskstats_extracts_block_device_counters() {
    let line = "   8       0 sda 412345 1234 8901234 12345 89012 456 34567890 23456 0 5678 23456 0 0 0 0\n";
    let map = parse_diskstats(line);
    let sda = map.get("sda").unwrap();
    assert_eq!(sda.reads_completed, 412345);
    assert_eq!(sda.sectors_read, 8901234);
    assert_eq!(sda.writes_completed, 89012);
    assert_eq!(sda.sectors_written, 34567890);
    assert_eq!(sda.io_time_ms, 5678);
}

#[test]
fn parse_diskstats_skips_loop_ram_zram_devices() {
    let content = "\
   7       0 loop0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n\
   1       0 ram0  0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n\
 252       0 zram0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n\
   8       0 sda   1 0 8 0 1 0 8 0 0 1 0 0 0 0 0 0\n";
    let map = parse_diskstats(content);
    assert!(!map.contains_key("loop0"));
    assert!(!map.contains_key("ram0"));
    assert!(!map.contains_key("zram0"));
    assert!(map.contains_key("sda"));
}

#[test]
fn parse_diskstats_handles_empty_content() {
    assert!(parse_diskstats("").is_empty());
}

#[test]
fn parse_diskstats_handles_short_lines() {
    // Lines with < 14 fields must be silently skipped
    assert!(parse_diskstats("   8       0 sda\n").is_empty());
}

// ── disk_sysfs_base_device_name ───────────────────────────────────────────────

#[test]
fn disk_sysfs_base_maps_partitions_to_whole_disk() {
    assert_eq!(disk_sysfs_base_device_name("sda1"), "sda");
    assert_eq!(disk_sysfs_base_device_name("sdaa2"), "sdaa");
    assert_eq!(disk_sysfs_base_device_name("nvme0n1p2"), "nvme0n1");
    assert_eq!(disk_sysfs_base_device_name("mmcblk0p1"), "mmcblk0");
    assert_eq!(disk_sysfs_base_device_name("xvda3"), "xvda");
}

#[test]
fn disk_sysfs_base_leaves_whole_disk_and_unknown_unchanged() {
    assert_eq!(disk_sysfs_base_device_name("sda"), "sda");
    assert_eq!(disk_sysfs_base_device_name("nvme0n1"), "nvme0n1");
    assert_eq!(disk_sysfs_base_device_name("mmcblk0"), "mmcblk0");
    assert_eq!(disk_sysfs_base_device_name("xvd3"), "xvd3");
    assert_eq!(disk_sysfs_base_device_name("dm-0"), "dm-0");
}

// ── parse_operstate ───────────────────────────────────────────────────────────

#[test]
fn parse_operstate_up_returns_true() {
    assert!(parse_operstate("up\n"));
    assert!(parse_operstate("up"));
    assert!(parse_operstate("unknown\n")); // loopback typically reports "unknown"
    assert!(parse_operstate("unknown"));
}

#[test]
fn parse_operstate_down_returns_false() {
    assert!(!parse_operstate("down\n"));
    assert!(!parse_operstate("dormant"));
    assert!(!parse_operstate("lowerlayerdown"));
    assert!(!parse_operstate(""));
    assert!(!parse_operstate("notpresent"));
}
