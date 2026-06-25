// Unit tests for the pure /sys/class/drm GPU parsers (no hardware required).

use homeserver::gpu_repo::{
    parse_busy_percent, parse_power_microwatts, parse_pwm_percent, parse_u64, parse_vendor_id,
};

#[test]
fn vendor_id_maps_amd_and_intel_only() {
    assert_eq!(parse_vendor_id("0x1002\n"), Some("amd"));
    assert_eq!(parse_vendor_id("0x8086"), Some("intel"));
    // NVIDIA is handled by NVML, so it is intentionally not mapped here.
    assert_eq!(parse_vendor_id("0x10de"), None);
    assert_eq!(parse_vendor_id("0xdead"), None);
    assert_eq!(parse_vendor_id(""), None);
}

#[test]
fn busy_percent_parses_and_clamps() {
    assert_eq!(parse_busy_percent("0\n"), Some(0.0));
    assert_eq!(parse_busy_percent("73"), Some(73.0));
    assert_eq!(parse_busy_percent("150"), Some(100.0)); // clamped
    assert!(parse_busy_percent("nope").is_none());
}

#[test]
fn u64_parses_byte_counters() {
    assert_eq!(parse_u64("8589934592\n"), Some(8_589_934_592));
    assert_eq!(parse_u64("0"), Some(0));
    assert!(parse_u64("-1").is_none());
    assert!(parse_u64("x").is_none());
}

#[test]
fn power_microwatts_to_watts() {
    assert_eq!(parse_power_microwatts("120000000\n"), Some(120.0));
    assert_eq!(parse_power_microwatts("0"), Some(0.0));
    assert!(parse_power_microwatts("bad").is_none());
}

#[test]
fn pwm_to_percent() {
    assert_eq!(parse_pwm_percent("0"), Some(0.0));
    assert_eq!(parse_pwm_percent("255\n"), Some(100.0));
    let half = parse_pwm_percent("128").unwrap();
    assert!((half - 50.196).abs() < 0.01);
    assert!(parse_pwm_percent("bad").is_none());
}
