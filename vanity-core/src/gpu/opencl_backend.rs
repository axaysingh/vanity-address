//! OpenCL runtime: compile kernels, batch grind, CPU re-verify matches.

use crate::chain::{ChainGrinder, GrindAttempt, KeypairResult};
use crate::gpu::pattern_pack::GpuPattern;
use crate::grinder::{CancelToken, GrindResult};
use crate::pattern::Pattern;
use opencl3::command_queue::CommandQueue;
use opencl3::context::Context;
use opencl3::device::{get_all_devices, Device, CL_DEVICE_TYPE_ALL, CL_DEVICE_TYPE_GPU};
use opencl3::kernel::{ExecuteKernel, Kernel};
use opencl3::memory::{
    Buffer, CL_MEM_COPY_HOST_PTR, CL_MEM_READ_ONLY, CL_MEM_READ_WRITE, CL_MEM_WRITE_ONLY,
};
use opencl3::program::Program;
use opencl3::types::{cl_int, cl_ulong, CL_BLOCKING};
use rand::rngs::OsRng;
use rand::RngCore;
use std::ptr;
use std::time::Instant;

const EVM_SRC: &str = include_str!("kernels/evm.cl");
const SOL_SRC: &str = include_str!("kernels/solana.cl");
// Affine secp256k1 is heavier per WI than broken Jacobian was — keep batches smaller.
const BATCH_EVM: usize = 16 * 1024;
const BATCH_SOL: usize = 16 * 1024; // ed25519 is heavier per work-item

fn device_ids() -> Result<Vec<opencl3::types::cl_device_id>, String> {
    // Prefer discrete/integrated GPUs; fall back to any OpenCL device (e.g. Apple).
    match get_all_devices(CL_DEVICE_TYPE_GPU) {
        Ok(ids) if !ids.is_empty() => Ok(ids),
        _ => get_all_devices(CL_DEVICE_TYPE_ALL).map_err(|e| format!("OpenCL devices: {e}")),
    }
}

pub fn enumerate_devices() -> Vec<crate::gpu::device::GpuDeviceInfo> {
    let Ok(ids) = device_ids() else {
        return Vec::new();
    };
    ids.into_iter()
        .enumerate()
        .filter_map(|(index, id)| {
            let d = Device::new(id);
            Some(crate::gpu::device::GpuDeviceInfo {
                index,
                name: d.name().ok()?,
                vendor: d.vendor().unwrap_or_default(),
                device_type: "GPU".into(),
                compute_units: d.max_compute_units().unwrap_or(0),
                max_clock_mhz: d.max_clock_frequency().unwrap_or(0),
            })
        })
        .collect()
}

fn first_gpu() -> Result<(Device, Context, CommandQueue), String> {
    let ids = device_ids()?;
    let id = *ids.first().ok_or("no OpenCL GPU devices found")?;
    let device = Device::new(id);
    let context = Context::from_device(&device).map_err(|e| format!("OpenCL context: {e}"))?;
    // Apple's OpenCL 1.2 loader lacks clCreateCommandQueueWithProperties.
    let queue = unsafe { CommandQueue::create(&context, device.id(), 0) }
        .map_err(|e| format!("OpenCL queue: {e}"))?;
    Ok((device, context, queue))
}

fn build_program(context: &Context, device: &Device, src: &str) -> Result<Program, String> {
    let mut program = Program::create_from_source(context, src)
        .map_err(|e| format!("OpenCL create program: {e}"))?;
    let devices = [device.id()];
    // Apple OpenCL is picky — empty options, then surface build log on failure.
    if let Err(e) = program.build(&devices, "") {
        let log = program
            .get_build_log(device.id())
            .unwrap_or_else(|_| "(no build log)".into());
        return Err(format!("OpenCL build failed: {e}\n{log}"));
    }
    Ok(program)
}

pub fn benchmark(chain_id: &str, duration_secs: f64) -> Result<(f64, String), String> {
    let id = normalize_id(chain_id)?;
    let (device, context, queue) = first_gpu()?;
    let name = device.name().unwrap_or_else(|_| "OpenCL GPU".into());
    let src = if id == "evm" { EVM_SRC } else { SOL_SRC };
    let program = build_program(&context, &device, src)?;
    let kernel_name = if id == "evm" {
        "evm_grind"
    } else {
        "sol_grind"
    };
    let kernel =
        Kernel::create(&program, kernel_name).map_err(|e| format!("OpenCL kernel: {e}"))?;

    let dummy = empty_pattern(id)?;
    let start = Instant::now();
    let mut attempts = 0u64;
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);
    let mut batch_base: u64 = 0;
    while start.elapsed().as_secs_f64() < duration_secs {
        let batch = if id == "sol" { BATCH_SOL } else { BATCH_EVM };
        run_batch(&context, &queue, &kernel, &seed, batch_base, &dummy, batch)?;
        attempts += batch as u64;
        batch_base = batch_base.wrapping_add(batch as u64);
        OsRng.fill_bytes(&mut seed);
    }
    let secs = start.elapsed().as_secs_f64().max(0.001);
    Ok((attempts as f64 / secs, name))
}

pub fn grind<G: ChainGrinder + Clone>(
    grinder: G,
    patterns: &[Pattern],
    cancel: &CancelToken,
    on_progress: impl Fn(u64, f64, f64) + Sync,
) -> Result<(GrindResult, crate::gpu::engine::GpuGrindReport), String> {
    let id = normalize_id(grinder.id())?;
    if patterns.is_empty() {
        return Err("at least one pattern is required".into());
    }
    let batch = if id == "sol" { BATCH_SOL } else { BATCH_EVM };
    // GPU MVP: single OR-alternative, no wildcards (enforced in GpuPattern).
    let packed = match id {
        "evm" => GpuPattern::for_evm(&patterns[0])?,
        "sol" => GpuPattern::for_solana(&patterns[0])?,
        _ => return Err(format!("GPU grind unsupported for chain '{id}'")),
    };

    let (device, context, queue) = first_gpu()?;
    let name = device.name().unwrap_or_else(|_| "OpenCL GPU".into());
    let src = if id == "evm" { EVM_SRC } else { SOL_SRC };
    let program = build_program(&context, &device, src)?;
    let kernel_name = if id == "evm" {
        "evm_grind"
    } else {
        "sol_grind"
    };
    let kernel =
        Kernel::create(&program, kernel_name).map_err(|e| format!("OpenCL kernel: {e}"))?;

    let expected = crate::pattern::expected_attempts_any(
        &patterns
            .iter()
            .map(|p| grinder.expected_attempts(p))
            .collect::<Vec<_>>(),
    );

    let start = Instant::now();
    let mut attempts = 0u64;
    let mut seed = [0u8; 32];
    let mut batch_base: u64 = 0;

    loop {
        if cancel.is_cancelled() {
            return Err("cancelled".into());
        }
        OsRng.fill_bytes(&mut seed);
        if let Some(sk) = run_batch(&context, &queue, &kernel, &seed, batch_base, &packed, batch)? {
            attempts += batch as u64;
            // CPU re-verify — never trust the GPU alone.
            let keypair = grinder.finalize(GrindAttempt::Secret32(sk));
            if patterns
                .iter()
                .any(|p| grinder.matches(&keypair.address, p))
            {
                let elapsed = start.elapsed().as_secs_f64();
                return Ok((
                    GrindResult {
                        keypair,
                        attempts,
                        elapsed_secs: elapsed,
                    },
                    crate::gpu::engine::GpuGrindReport {
                        device_name: name,
                        backend: "opencl",
                        attempts,
                        elapsed_secs: elapsed,
                    },
                ));
            }
            // False positive — keep going.
        } else {
            attempts += batch as u64;
        }
        batch_base = batch_base.wrapping_add(batch as u64);
        let secs = start.elapsed().as_secs_f64().max(0.001);
        let rate = attempts as f64 / secs;
        let eta_min = (expected - attempts as f64).max(0.0) / rate / 60.0;
        on_progress(attempts, rate, eta_min);
    }
}

fn normalize_id(id: &str) -> Result<&'static str, String> {
    let id = id.to_ascii_lowercase();
    match id.as_str() {
        "evm" | "eth" | "ethereum" => Ok("evm"),
        "sol" | "solana" => Ok("sol"),
        other => Err(format!(
            "OpenCL GPU MVP supports sol and evm only (got '{other}')"
        )),
    }
}

fn empty_pattern(id: &str) -> Result<GpuPattern, String> {
    let p = Pattern {
        prefix: String::new(),
        suffix: String::new(),
        contains: String::new(),
        prefix_match: String::new(),
        suffix_match: String::new(),
        contains_match: String::new(),
        ignore_case: true,
        fixed_prefix_len: 0,
    };
    if id == "evm" {
        GpuPattern::for_evm(&p)
    } else {
        GpuPattern::for_solana(&p)
    }
}

fn run_batch(
    context: &Context,
    queue: &CommandQueue,
    kernel: &Kernel,
    seed: &[u8; 32],
    batch_base: u64,
    pat: &GpuPattern,
    global: usize,
) -> Result<Option<[u8; 32]>, String> {
    let mut found: cl_int = -1;
    let mut out_sk = [0u8; 32];
    let batch_base_arg = batch_base as cl_ulong;

    // Pass GpuPattern as one buffer — Apple OpenCL mis-bound interleaved scalar +
    // __constant pointer args (prefix_len was read as 0 → every key "matched").
    let seed_buf = unsafe {
        Buffer::<u8>::create(
            context,
            CL_MEM_READ_ONLY | CL_MEM_COPY_HOST_PTR,
            32,
            seed.as_ptr() as *mut _,
        )
    }
    .map_err(|e| format!("seed buf: {e}"))?;
    let pat_buf = unsafe {
        Buffer::<u8>::create(
            context,
            CL_MEM_READ_ONLY | CL_MEM_COPY_HOST_PTR,
            std::mem::size_of::<GpuPattern>(),
            (pat as *const GpuPattern as *mut u8) as *mut _,
        )
    }
    .map_err(|e| format!("pat buf: {e}"))?;
    let found_buf = unsafe {
        Buffer::<cl_int>::create(
            context,
            CL_MEM_READ_WRITE | CL_MEM_COPY_HOST_PTR,
            1,
            &mut found as *mut cl_int as *mut _,
        )
    }
    .map_err(|e| format!("found buf: {e}"))?;
    let sk_buf = unsafe { Buffer::<u8>::create(context, CL_MEM_WRITE_ONLY, 32, ptr::null_mut()) }
        .map_err(|e| format!("sk buf: {e}"))?;

    unsafe {
        ExecuteKernel::new(kernel)
            .set_arg(&seed_buf)
            .set_arg(&batch_base_arg)
            .set_arg(&pat_buf)
            .set_arg(&found_buf)
            .set_arg(&sk_buf)
            .set_global_work_size(global)
            .enqueue_nd_range(queue)
            .map_err(|e| format!("enqueue: {e}"))?;
    }
    queue.finish().map_err(|e| format!("finish: {e}"))?;

    unsafe {
        queue
            .enqueue_read_buffer(
                &found_buf,
                CL_BLOCKING,
                0,
                std::slice::from_mut(&mut found),
                &[],
            )
            .map_err(|e| format!("read found: {e}"))?;
    }
    if found < 0 {
        return Ok(None);
    }
    unsafe {
        queue
            .enqueue_read_buffer(&sk_buf, CL_BLOCKING, 0, &mut out_sk, &[])
            .map_err(|e| format!("read sk: {e}"))?;
    }
    Ok(Some(out_sk))
}

// Silence unused import if KeypairResult unused in some cfgs.
#[allow(dead_code)]
fn _touch(_: &KeypairResult) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::{ChainGrinder, GrindAttempt};
    use crate::grinder::CancelToken;

    #[test]
    fn builds_evm_kernel() {
        let (device, context, _q) = first_gpu().expect("gpu");
        build_program(&context, &device, EVM_SRC).expect("evm build");
    }

    #[test]
    fn builds_sol_kernel() {
        let (device, context, _q) = first_gpu().expect("gpu");
        build_program(&context, &device, SOL_SRC).expect("sol build");
    }

    #[test]
    fn evm_gpu_sk1_and_sk2_addresses() {
        use crate::chains::EvmGrinder;
        let g = EvmGrinder;
        let dump_src = format!("#define VANITY_VECTOR_SK 1\n{EVM_SRC}");
        let (device, context, queue) = first_gpu().expect("gpu");
        let program = build_program(&context, &device, &dump_src).expect("build");
        let kernel = Kernel::create(&program, "evm_grind").expect("kernel");
        let packed = empty_pattern("evm").unwrap();
        for (lo, _) in [(1u8, ()), (2u8, ())] {
            let mut sk = [0u8; 32];
            sk[31] = lo;
            let expect = g
                .finalize(GrindAttempt::Secret32(sk))
                .address
                .to_ascii_lowercase();
            let mut seed = [0u8; 32];
            seed[31] = lo;
            let got = run_batch(&context, &queue, &kernel, &seed, 0, &packed, 64)
                .unwrap()
                .unwrap();
            let addr = format!("0x{}", hex::encode(&got[..20]));
            assert_eq!(addr, expect, "sk={lo}");
        }
    }

    #[test]
    fn evm_finds_short_prefix() {
        use crate::chains::EvmGrinder;
        let g = EvmGrinder;
        let pat = g.build_pattern(Some("a"), None, None, false).unwrap();
        let cancel = CancelToken::new();
        let (result, report) = grind(g, &[pat], &cancel, |_, _, _| {}).expect("gpu grind");
        assert!(
            result
                .keypair
                .address
                .to_ascii_lowercase()
                .starts_with("0xa"),
            "{}",
            result.keypair.address
        );
        assert!(report.attempts > 0);
        println!(
            "found {} on {} in {:.2}s ({} attempts)",
            result.keypair.address, report.device_name, result.elapsed_secs, result.attempts
        );
    }

    #[test]
    fn evm_finds_three_char_prefix() {
        use crate::chains::EvmGrinder;
        let g = EvmGrinder;
        let pat = g.build_pattern(Some("abc"), None, None, false).unwrap();
        assert_eq!(GpuPattern::for_evm(&pat).unwrap().prefix_len, 3);
        let cancel = CancelToken::new();
        let (result, report) = grind(g, &[pat], &cancel, |_, _, _| {}).expect("gpu grind abc");
        let addr = result.keypair.address.to_ascii_lowercase();
        assert!(
            addr.starts_with("0xabc"),
            "expected 0xabc… got {addr} after {} attempts",
            result.attempts
        );
        assert!(
            result.attempts < 5_000_000,
            "took {} attempts — pattern match likely broken again",
            result.attempts
        );
        println!(
            "found {} on {} in {:.2}s ({} attempts)",
            result.keypair.address, report.device_name, result.elapsed_secs, result.attempts
        );
    }

    #[test]
    fn evm_gpu_hit_passes_cpu_reverify() {
        use crate::chains::EvmGrinder;
        let g = EvmGrinder;
        let pat = g.build_pattern(Some("ab"), None, None, false).unwrap();
        let packed = GpuPattern::for_evm(&pat).unwrap();
        let (device, context, queue) = first_gpu().expect("gpu");
        let program = build_program(&context, &device, EVM_SRC).expect("build");
        let kernel = Kernel::create(&program, "evm_grind").expect("kernel");
        let mut seed = [0u8; 32];
        OsRng.fill_bytes(&mut seed);
        let sk = run_batch(&context, &queue, &kernel, &seed, 0, &packed, BATCH_EVM)
            .unwrap()
            .expect("gpu hit");
        let addr = g
            .finalize(GrindAttempt::Secret32(sk))
            .address
            .to_ascii_lowercase();
        assert!(
            addr.starts_with("0xab"),
            "GPU hit failed CPU re-verify: {addr}"
        );
    }

    #[test]
    fn sol_finds_short_prefix() {
        use crate::chains::SolanaGrinder;
        let g = SolanaGrinder;
        let pat = g.build_pattern(Some("A"), None, None, true).unwrap();
        let cancel = CancelToken::new();
        let (result, report) = grind(g, &[pat], &cancel, |_, _, _| {}).expect("sol gpu grind");
        assert!(
            result.keypair.address.starts_with('A'),
            "{}",
            result.keypair.address
        );
        assert!(report.attempts > 0);
        println!(
            "found {} on {} in {:.2}s ({} attempts)",
            result.keypair.address, report.device_name, result.elapsed_secs, result.attempts
        );
    }
}
