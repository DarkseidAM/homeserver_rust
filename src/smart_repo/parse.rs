// Pure parsers for `smartctl --json` output. No I/O, so unit-testable with captured samples.

use crate::models::SmartHealth;
use serde_json::Value;

/// Parse `smartctl --scan -j` output into a list of device paths.
pub fn parse_scan_devices(json: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    v["devices"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|d| d["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Parse `smartctl -a -j <device>` output into a [`SmartHealth`]. Returns None only if the JSON
/// is unparseable; missing individual fields degrade to None/false/empty.
pub fn parse_smartctl_json(json: &str, device: &str) -> Option<SmartHealth> {
    let v: Value = serde_json::from_str(json).ok()?;
    Some(SmartHealth {
        device: device.to_string(),
        model: v["model_name"].as_str().unwrap_or("").to_string(),
        health_passed: v["smart_status"]["passed"].as_bool().unwrap_or(false),
        temperature_c: v["temperature"]["current"].as_i64(),
        power_on_hours: v["power_on_time"]["hours"].as_u64(),
        reallocated_sectors: ata_attr_raw(&v, 5),
        wear_level_percent: wear_level(&v),
    })
}

/// Raw value of an ATA SMART attribute by id (from `ata_smart_attributes.table`).
fn ata_attr_raw(v: &Value, id: u64) -> Option<u64> {
    v["ata_smart_attributes"]["table"]
        .as_array()?
        .iter()
        .find(|a| a["id"].as_u64() == Some(id))
        .and_then(|a| a["raw"]["value"].as_u64())
}

/// SSD/NVMe life used as a percentage. Prefer NVMe `percentage_used`; else derive from an ATA
/// wear-leveling attribute's normalized value (which counts down from 100 = full life remaining).
fn wear_level(v: &Value) -> Option<u8> {
    if let Some(used) = v["nvme_smart_health_information_log"]["percentage_used"].as_u64() {
        return Some(used.min(100) as u8);
    }
    let table = v["ata_smart_attributes"]["table"].as_array()?;
    table
        .iter()
        .find(|a| matches!(a["id"].as_u64(), Some(177) | Some(231) | Some(233)))
        .and_then(|a| a["value"].as_u64())
        .map(|life_remaining| (100u64.saturating_sub(life_remaining)).min(100) as u8)
}
