// Pure metric extraction + comparison for alert rules. No I/O — unit-testable.

use crate::models::FullSystemSnapshot;

/// Extract a named scalar metric from a snapshot. Returns None if unavailable
/// (e.g. no swap, no GPUs, no partitions).
pub fn extract_metric(metric: &str, s: &FullSystemSnapshot) -> Option<f64> {
    match metric {
        "cpu_usage" => Some(s.cpu.usage_percent),
        "mem_usage_percent" => Some(s.ram.usage_percent),
        "swap_usage_percent" => {
            if s.ram.swap_total > 0 {
                Some(s.ram.swap_used as f64 / s.ram.swap_total as f64 * 100.0)
            } else {
                None
            }
        }
        "load_avg_1" => Some(s.system.load_avg_1),
        "cpu_temperature" => Some(s.cpu.temperature),
        "disk_usage_percent" => s
            .storage
            .partitions
            .iter()
            .map(|p| p.usage_percent)
            .fold(None, fold_max),
        "gpu_temperature" => s.gpus.iter().map(|g| g.temperature_c).fold(None, fold_max),
        "gpu_utilization" => s
            .gpus
            .iter()
            .map(|g| g.utilization_percent)
            .fold(None, fold_max),
        _ => None,
    }
}

/// Evaluate `value op threshold`.
pub fn compare(value: f64, op: &str, threshold: f64) -> bool {
    match op {
        ">" => value > threshold,
        ">=" => value >= threshold,
        "<" => value < threshold,
        "<=" => value <= threshold,
        _ => false,
    }
}

fn fold_max(acc: Option<f64>, v: f64) -> Option<f64> {
    Some(match acc {
        Some(m) => m.max(v),
        None => v,
    })
}
