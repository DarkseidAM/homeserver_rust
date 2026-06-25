// Unit tests for the pure smartctl JSON parsers (no smartctl/hardware required).

use homeserver::smart_repo::{parse_scan_devices, parse_smartctl_json};

#[test]
fn scan_devices_extracts_paths() {
    let json =
        r#"{"devices":[{"name":"/dev/sda","type":"sat"},{"name":"/dev/nvme0","type":"nvme"}]}"#;
    assert_eq!(parse_scan_devices(json), vec!["/dev/sda", "/dev/nvme0"]);
}

#[test]
fn scan_devices_empty_or_invalid() {
    assert!(parse_scan_devices("{}").is_empty());
    assert!(parse_scan_devices("not json").is_empty());
    assert!(parse_scan_devices(r#"{"devices":[]}"#).is_empty());
}

#[test]
fn parse_ata_ssd_fields() {
    // Minimal ATA SSD shape: health, temperature, power-on hours, reallocated (id 5),
    // and wear-leveling (id 177) normalized value = remaining life.
    let json = r#"{
        "model_name": "Samsung SSD 860",
        "smart_status": {"passed": true},
        "temperature": {"current": 41},
        "power_on_time": {"hours": 9001},
        "ata_smart_attributes": {"table": [
            {"id": 5, "name": "Reallocated_Sector_Ct", "value": 100, "raw": {"value": 0}},
            {"id": 177, "name": "Wear_Leveling_Count", "value": 94, "raw": {"value": 88}}
        ]}
    }"#;
    let h = parse_smartctl_json(json, "/dev/sda").unwrap();
    assert_eq!(h.device, "/dev/sda");
    assert_eq!(h.model, "Samsung SSD 860");
    assert!(h.health_passed);
    assert_eq!(h.temperature_c, Some(41));
    assert_eq!(h.power_on_hours, Some(9001));
    assert_eq!(h.reallocated_sectors, Some(0));
    // 100 - 94 (remaining life) = 6% used.
    assert_eq!(h.wear_level_percent, Some(6));
}

#[test]
fn parse_nvme_uses_percentage_used() {
    let json = r#"{
        "model_name": "WD Black SN850",
        "smart_status": {"passed": true},
        "temperature": {"current": 38},
        "power_on_time": {"hours": 500},
        "nvme_smart_health_information_log": {"percentage_used": 12}
    }"#;
    let h = parse_smartctl_json(json, "/dev/nvme0").unwrap();
    assert_eq!(h.model, "WD Black SN850");
    assert_eq!(h.wear_level_percent, Some(12));
    assert_eq!(h.reallocated_sectors, None);
}

#[test]
fn parse_failing_health_and_missing_fields() {
    let json = r#"{"smart_status": {"passed": false}}"#;
    let h = parse_smartctl_json(json, "/dev/sdb").unwrap();
    assert!(!h.health_passed);
    assert_eq!(h.model, "");
    assert_eq!(h.temperature_c, None);
    assert_eq!(h.wear_level_percent, None);
}

#[test]
fn parse_invalid_json_returns_none() {
    assert!(parse_smartctl_json("definitely not json", "/dev/sda").is_none());
}
