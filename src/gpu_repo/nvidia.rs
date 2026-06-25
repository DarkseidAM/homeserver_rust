// NVIDIA GPU stats via NVML (feature `gpu-nvidia`). libnvidia-ml is loaded at runtime;
// absent driver/GPU → init returns None and no NVIDIA GPUs are reported.

use crate::models::GpuStats;
use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;

/// Owns the initialized NVML handle for the process lifetime.
pub struct NvmlHandle(Nvml);

/// Initialize NVML once. Returns None if NVML/driver is unavailable (logged at debug).
pub fn init() -> Option<NvmlHandle> {
    match Nvml::init() {
        Ok(nvml) => Some(NvmlHandle(nvml)),
        Err(e) => {
            tracing::debug!(error = %e, "NVML init failed (no NVIDIA driver/GPU?)");
            None
        }
    }
}

/// Query all NVIDIA devices. Per-field failures degrade to 0/None rather than dropping the GPU.
pub fn collect(handle: &NvmlHandle) -> Vec<GpuStats> {
    let nvml = &handle.0;
    let count = match nvml.device_count() {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!(error = %e, "NVML device_count failed");
            return Vec::new();
        }
    };

    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        let Ok(dev) = nvml.device_by_index(i) else {
            continue;
        };
        let name = dev.name().unwrap_or_else(|_| "NVIDIA GPU".to_string());
        let util = dev.utilization_rates().ok();
        let mem = dev.memory_info().ok();
        let temperature_c = dev
            .temperature(TemperatureSensor::Gpu)
            .map(|t| t as f64)
            .unwrap_or(0.0);
        let power_watts = dev.power_usage().ok().map(|mw| mw as f64 / 1000.0);
        let fan_percent = dev.fan_speed(0).ok().map(|p| p as f64);

        out.push(GpuStats {
            index: i,
            vendor: "nvidia".to_string(),
            name,
            utilization_percent: util.map(|u| u.gpu as f64).unwrap_or(0.0),
            memory_used_bytes: mem.as_ref().map(|m| m.used).unwrap_or(0),
            memory_total_bytes: mem.as_ref().map(|m| m.total).unwrap_or(0),
            temperature_c,
            power_watts,
            fan_percent,
        });
    }
    out
}
