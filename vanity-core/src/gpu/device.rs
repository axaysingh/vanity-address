//! OpenCL device discovery (feature-gated).

#[derive(Debug, Clone)]
pub struct GpuDeviceInfo {
    pub index: usize,
    pub name: String,
    pub vendor: String,
    pub device_type: String,
    pub compute_units: u32,
    pub max_clock_mhz: u32,
}

/// True when the binary was built with `opencl` and at least one device enumerates.
pub fn gpu_available() -> bool {
    !list_gpu_devices().is_empty()
}

#[cfg(feature = "opencl")]
pub fn list_gpu_devices() -> Vec<GpuDeviceInfo> {
    crate::gpu::opencl_backend::enumerate_devices()
}

#[cfg(not(feature = "opencl"))]
pub fn list_gpu_devices() -> Vec<GpuDeviceInfo> {
    Vec::new()
}
