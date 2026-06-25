// GPU metrics collector. NVIDIA via NVML (feature `gpu-nvidia`); AMD/Intel via /sys/class/drm.
// collect() is cheap (a few small sysfs reads + NVML queries) and is called inline per worker tick.

#[cfg(feature = "gpu-nvidia")]
mod nvidia;
mod sysfs;

// Pure sysfs parsers re-exported for unit tests.
pub use sysfs::{
    parse_busy_percent, parse_power_microwatts, parse_pwm_percent, parse_u64, parse_vendor_id,
};

use crate::models::GpuStats;

/// Aggregates GPU stats across available backends.
pub struct GpuRepo {
    #[cfg(feature = "gpu-nvidia")]
    nvml: Option<nvidia::NvmlHandle>,
}

impl GpuRepo {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "gpu-nvidia")]
            nvml: nvidia::init(),
        }
    }

    /// Collect from all backends. Never errors; missing/unsupported hardware yields fewer entries.
    pub fn collect(&self) -> Vec<GpuStats> {
        let mut out = Vec::new();
        #[cfg(feature = "gpu-nvidia")]
        if let Some(handle) = &self.nvml {
            out.extend(nvidia::collect(handle));
        }
        out.extend(sysfs::collect());
        out
    }
}

impl Default for GpuRepo {
    fn default() -> Self {
        Self::new()
    }
}
