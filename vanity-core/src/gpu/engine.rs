//! High-level GPU grind entry points (CPU fallback is the caller's job).

use crate::chain::ChainGrinder;
use crate::grinder::{CancelToken, GrindResult};
use crate::pattern::Pattern;
use crate::system::SystemProfile;

/// How aggressively to prefer the OpenCL path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GpuMode {
    /// Never use GPU.
    #[default]
    Off,
    /// Use GPU when built with `opencl`, a device exists, and the chain is supported.
    Auto,
    /// Require GPU; error if unavailable / unsupported.
    Require,
}

impl GpuMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "off" | "cpu" | "no" | "false" | "0" => Ok(Self::Off),
            "auto" | "on" | "yes" | "true" | "1" | "gpu" => Ok(Self::Auto),
            "require" | "force" | "only" | "gpu-only" | "gpu_only" => Ok(Self::Require),
            other => Err(format!(
                "Unknown GPU mode '{other}'. Use: off, auto, require"
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GpuGrindReport {
    pub device_name: String,
    pub backend: &'static str,
    pub attempts: u64,
    pub elapsed_secs: f64,
}

/// Run a short OpenCL warm-up; `Err` if GPU unavailable.
pub fn benchmark_gpu(chain_id: &str, duration_secs: f64) -> Result<(f64, String), String> {
    #[cfg(feature = "opencl")]
    {
        crate::gpu::opencl_backend::benchmark(chain_id, duration_secs)
    }
    #[cfg(not(feature = "opencl"))]
    {
        let _ = (chain_id, duration_secs);
        Err("this build was compiled without OpenCL (enable the `opencl` feature)".into())
    }
}

/// Grind on GPU. On success the keypair has already been CPU-verified.
pub fn grind_gpu<G: ChainGrinder + Clone + Send + Sync + 'static>(
    grinder: G,
    patterns: &[Pattern],
    _profile: &SystemProfile,
    cancel: &CancelToken,
    on_progress: impl Fn(u64, f64, f64) + Sync,
) -> Result<(GrindResult, GpuGrindReport), String> {
    #[cfg(feature = "opencl")]
    {
        crate::gpu::opencl_backend::grind(grinder, patterns, cancel, on_progress)
    }
    #[cfg(not(feature = "opencl"))]
    {
        let _ = (grinder, patterns, cancel, on_progress);
        Err("this build was compiled without OpenCL (enable the `opencl` feature)".into())
    }
}
