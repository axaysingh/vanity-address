//! OpenCL GPU grind (Solana + EVM) with mandatory CPU re-verify of any match.
//!
//! Enabled with the `opencl` feature. When the feature is off, or no device is
//! present, callers fall back to the rayon CPU grinder.

mod device;
mod engine;
mod pattern_pack;

#[cfg(feature = "opencl")]
mod opencl_backend;

pub use device::{gpu_available, list_gpu_devices, GpuDeviceInfo};
pub use engine::{benchmark_gpu, grind_gpu, GpuGrindReport, GpuMode};

/// Chains the OpenCL MVP can accelerate. Everything else stays on CPU.
pub fn gpu_supports_chain(chain_id: &str) -> bool {
    use crate::chain::ChainGrinder;
    match crate::chains::Chain::from_id(chain_id) {
        Ok(c) => c.id() == "evm" || c.id() == "sol",
        Err(_) => matches!(
            chain_id.to_ascii_lowercase().as_str(),
            "sol" | "solana" | "evm" | "eth" | "ethereum"
        ),
    }
}
