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
use rayon::prelude::*;
use secp256k1::{PublicKey, Scalar, Secp256k1, SecretKey};
use std::ptr;
use std::time::Instant;

const EVM_SRC: &str = include_str!("kernels/evm.cl");
const SOL_SRC: &str = include_str!("kernels/solana.cl");
/// Points sharing one field inversion (Montgomery batch inversion) in the EVM kernel.
const EVM_GROUP: usize = 64;
/// Groups each work-item walks per launch.
const EVM_GROUPS: usize = 4;
/// Work-items; each owns a random start point that persists on the device between launches.
const EVM_ITEMS: usize = 8 * 1024;
/// Scalar distance every work-item advances per launch.
const EVM_STEPS: u64 = (EVM_GROUP * EVM_GROUPS) as u64;
/// Candidates tested per launch.
const EVM_PER_LAUNCH: u64 = EVM_ITEMS as u64 * EVM_STEPS;
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
    if id == "evm" {
        return evm_benchmark(duration_secs);
    }
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

    let dummy = never_pattern(id)?;
    let start = Instant::now();
    let mut attempts = 0u64;
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);
    let mut batch_base: u64 = 0;
    while start.elapsed().as_secs_f64() < duration_secs {
        let batch = BATCH_SOL;
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
    if id == "evm" {
        return grind_evm(grinder, patterns, cancel, on_progress);
    }
    let batch = BATCH_SOL;
    // GPU MVP: single OR-alternative, no wildcards (enforced in GpuPattern).
    let packed = GpuPattern::for_solana(&patterns[0])?;

    let (device, context, queue) = first_gpu()?;
    let name = device.name().unwrap_or_else(|_| "OpenCL GPU".into());
    let program = build_program(&context, &device, SOL_SRC)?;
    let kernel =
        Kernel::create(&program, "sol_grind").map_err(|e| format!("OpenCL kernel: {e}"))?;

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

// ---------------------------------------------------------------------------
// EVM: incremental search with persistent per-work-item start points
// ---------------------------------------------------------------------------

fn evm_source() -> String {
    format!("#define GROUP {EVM_GROUP}\n#define GROUPS {EVM_GROUPS}\n{EVM_SRC}")
}

/// 32 big-endian bytes → eight little-endian u32 limbs (the kernel's `fe`).
fn fe_limbs(be: &[u8]) -> [u32; 8] {
    let mut d = [0u32; 8];
    for (i, limb) in d.iter_mut().enumerate() {
        let o = 28 - 4 * i;
        *limb = u32::from_be_bytes([be[o], be[o + 1], be[o + 2], be[o + 3]]);
    }
    d
}

fn point_limbs(pk: &PublicKey) -> ([u32; 8], [u32; 8]) {
    let raw = pk.serialize_uncompressed(); // 0x04 ‖ X ‖ Y
    (fe_limbs(&raw[1..33]), fe_limbs(&raw[33..65]))
}

struct EvmSession {
    queue: CommandQueue,
    context: Context,
    kernel: Kernel,
    pat_buf: Buffer<u8>,
    found_buf: Buffer<cl_int>,
    off_buf: Buffer<u32>,
    bx: Buffer<u32>,
    by: Buffer<u32>,
    tabx: Buffer<u32>,
    taby: Buffer<u32>,
    /// Start scalar of each work-item (the device only holds the current point).
    k0: Vec<[u8; 32]>,
    launches: u64,
}

impl EvmSession {
    fn new(pat: &GpuPattern) -> Result<(Self, String), String> {
        let (device, context, queue) = first_gpu()?;
        let name = device.name().unwrap_or_else(|_| "OpenCL GPU".into());
        let program = build_program(&context, &device, &evm_source())?;
        let kernel =
            Kernel::create(&program, "evm_grind").map_err(|e| format!("OpenCL kernel: {e}"))?;

        let secp = Secp256k1::new();
        // Table of j·G, j = 1..=GROUP.
        let mut tabx = Vec::with_capacity(EVM_GROUP * 8);
        let mut taby = Vec::with_capacity(EVM_GROUP * 8);
        for j in 1..=EVM_GROUP {
            let mut be = [0u8; 32];
            be[24..].copy_from_slice(&(j as u64).to_be_bytes());
            let sk = SecretKey::from_slice(&be).map_err(|e| format!("table scalar: {e}"))?;
            let (x, y) = point_limbs(&PublicKey::from_secret_key(&secp, &sk));
            tabx.extend_from_slice(&x);
            taby.extend_from_slice(&y);
        }

        let mut session = Self {
            pat_buf: make_buf(&context, CL_MEM_READ_ONLY, pat_bytes(pat))?,
            found_buf: make_buf(&context, CL_MEM_READ_WRITE, vec![-1 as cl_int])?,
            off_buf: make_buf(&context, CL_MEM_READ_WRITE, vec![0u32; 2])?,
            bx: make_buf(&context, CL_MEM_READ_WRITE, vec![0u32; EVM_ITEMS * 8])?,
            by: make_buf(&context, CL_MEM_READ_WRITE, vec![0u32; EVM_ITEMS * 8])?,
            tabx: make_buf(&context, CL_MEM_READ_ONLY, tabx)?,
            taby: make_buf(&context, CL_MEM_READ_ONLY, taby)?,
            k0: Vec::new(),
            launches: 0,
            queue,
            context,
            kernel,
        };
        session.reseed()?;
        Ok((session, name))
    }

    /// Fresh random start point for every work-item (also used after a rejected hit).
    fn reseed(&mut self) -> Result<(), String> {
        let secp = Secp256k1::new();
        let starts: Vec<([u8; 32], [u32; 8], [u32; 8])> = (0..EVM_ITEMS)
            .into_par_iter()
            .map(|_| {
                let sk = SecretKey::new(&mut OsRng);
                let (x, y) = point_limbs(&PublicKey::from_secret_key(&secp, &sk));
                (sk.secret_bytes(), x, y)
            })
            .collect();
        let mut xs = Vec::with_capacity(EVM_ITEMS * 8);
        let mut ys = Vec::with_capacity(EVM_ITEMS * 8);
        self.k0.clear();
        for (k, x, y) in starts {
            self.k0.push(k);
            xs.extend_from_slice(&x);
            ys.extend_from_slice(&y);
        }
        write_buf(&self.queue, &mut self.bx, &xs)?;
        write_buf(&self.queue, &mut self.by, &ys)?;
        self.launches = 0;
        Ok(())
    }

    /// Run one launch; `Some(sk)` is a GPU hit (still needs CPU re-verify).
    fn launch(&mut self) -> Result<Option<[u8; 32]>, String> {
        write_buf(&self.queue, &mut self.found_buf, &[-1 as cl_int])?;
        write_buf(&self.queue, &mut self.off_buf, &[0u32; 2])?;
        unsafe {
            ExecuteKernel::new(&self.kernel)
                .set_arg(&self.pat_buf)
                .set_arg(&self.found_buf)
                .set_arg(&self.off_buf)
                .set_arg(&self.bx)
                .set_arg(&self.by)
                .set_arg(&self.tabx)
                .set_arg(&self.taby)
                .set_global_work_size(EVM_ITEMS)
                .enqueue_nd_range(&self.queue)
                .map_err(|e| format!("enqueue: {e}"))?;
        }
        self.queue.finish().map_err(|e| format!("finish: {e}"))?;

        let mut found: [cl_int; 1] = [-1];
        read_buf(&self.queue, &self.found_buf, &mut found)?;
        let launch_start = self.launches * EVM_STEPS;
        self.launches += 1;
        let mut off = [0u32; 2];
        read_buf(&self.queue, &self.off_buf, &mut off)?;
        if found[0] < 0 {
            if off[1] != 0 {
                // A work-item hit B == ±j·G (astronomically rare); its state is invalid.
                self.reseed()?;
            }
            return Ok(None);
        }
        let item = found[0] as usize;
        let k0 = self
            .k0
            .get(item)
            .ok_or("GPU returned an out-of-range work-item")?;
        let mut tweak = [0u8; 32];
        tweak[24..].copy_from_slice(&(launch_start + u64::from(off[0])).to_be_bytes());
        let tweak = Scalar::from_be_bytes(tweak).map_err(|_| "tweak out of range".to_string())?;
        let sk = SecretKey::from_slice(k0)
            .and_then(|k| k.add_tweak(&tweak))
            .map_err(|e| format!("secret reconstruction: {e}"))?;
        Ok(Some(sk.secret_bytes()))
    }

    #[allow(dead_code)]
    fn context(&self) -> &Context {
        &self.context
    }
}

fn pat_bytes(pat: &GpuPattern) -> Vec<u8> {
    let n = std::mem::size_of::<GpuPattern>();
    unsafe { std::slice::from_raw_parts(pat as *const GpuPattern as *const u8, n) }.to_vec()
}

fn make_buf<T>(context: &Context, flags: u64, mut data: Vec<T>) -> Result<Buffer<T>, String> {
    unsafe {
        Buffer::<T>::create(
            context,
            flags | CL_MEM_COPY_HOST_PTR,
            data.len(),
            data.as_mut_ptr() as *mut _,
        )
    }
    .map_err(|e| format!("buffer: {e}"))
}

fn write_buf<T>(queue: &CommandQueue, buf: &mut Buffer<T>, data: &[T]) -> Result<(), String> {
    unsafe { queue.enqueue_write_buffer(buf, CL_BLOCKING, 0, data, &[]) }
        .map(|_| ())
        .map_err(|e| format!("write buffer: {e}"))
}

fn read_buf<T>(queue: &CommandQueue, buf: &Buffer<T>, out: &mut [T]) -> Result<(), String> {
    unsafe { queue.enqueue_read_buffer(buf, CL_BLOCKING, 0, out, &[]) }
        .map(|_| ())
        .map_err(|e| format!("read buffer: {e}"))
}

fn evm_benchmark(duration_secs: f64) -> Result<(f64, String), String> {
    let (mut session, name) = EvmSession::new(&never_pattern("evm")?)?;
    let start = Instant::now();
    let mut attempts = 0u64;
    while start.elapsed().as_secs_f64() < duration_secs {
        session.launch()?;
        attempts += EVM_PER_LAUNCH;
    }
    Ok((
        attempts as f64 / start.elapsed().as_secs_f64().max(0.001),
        name,
    ))
}

fn grind_evm<G: ChainGrinder + Clone>(
    grinder: G,
    patterns: &[Pattern],
    cancel: &CancelToken,
    on_progress: impl Fn(u64, f64, f64) + Sync,
) -> Result<(GrindResult, crate::gpu::engine::GpuGrindReport), String> {
    // GPU MVP: single OR-alternative, no wildcards (enforced in GpuPattern).
    let packed = GpuPattern::for_evm(&patterns[0])?;
    let (mut session, name) = EvmSession::new(&packed)?;
    let expected = crate::pattern::expected_attempts_any(
        &patterns
            .iter()
            .map(|p| grinder.expected_attempts(p))
            .collect::<Vec<_>>(),
    );

    let start = Instant::now();
    let mut attempts = 0u64;
    let mut rejected = 0u32;
    loop {
        if cancel.is_cancelled() {
            return Err("cancelled".into());
        }
        let hit = session.launch()?;
        attempts += EVM_PER_LAUNCH;
        if let Some(sk) = hit {
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
            rejected += 1;
            if rejected > 8 {
                return Err("GPU kernel produced repeated invalid hits; use --gpu off".into());
            }
            // Device state is stale after an early exit — start from fresh points.
            session.reseed()?;
        }
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

/// A pattern no address can match, so benchmarks measure real work (an empty
/// pattern matches instantly and makes every other work-item exit early).
fn never_pattern(id: &str) -> Result<GpuPattern, String> {
    let (fill, ignore_case) = if id == "evm" {
        ("f", true)
    } else {
        ("z", false)
    };
    let never = fill.repeat(32);
    let p = Pattern {
        prefix: never.clone(),
        suffix: String::new(),
        contains: String::new(),
        prefix_match: never,
        suffix_match: String::new(),
        contains_match: String::new(),
        ignore_case,
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
    use crate::chain::ChainGrinder;
    use crate::grinder::CancelToken;

    /// Linux CI / headless hosts often have OpenCL headers but no usable device.
    /// Skip rather than fail so `cargo test -p vanity-address` stays green there.
    fn require_gpu() -> bool {
        match first_gpu() {
            Ok(_) => true,
            Err(e) => {
                eprintln!("skipping OpenCL GPU test: {e}");
                false
            }
        }
    }

    #[test]
    fn builds_evm_kernel() {
        if !require_gpu() {
            return;
        }
        let (device, context, _q) = first_gpu().expect("gpu");
        build_program(&context, &device, &evm_source()).expect("evm build");
    }

    #[test]
    fn builds_sol_kernel() {
        if !require_gpu() {
            return;
        }
        let (device, context, _q) = first_gpu().expect("gpu");
        build_program(&context, &device, SOL_SRC).expect("sol build");
    }

    #[test]
    fn evm_finds_short_prefix() {
        if !require_gpu() {
            return;
        }
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
        if !require_gpu() {
            return;
        }
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
    fn evm_hit_after_many_launches_reconstructs_key() {
        if !require_gpu() {
            return;
        }
        use crate::chains::EvmGrinder;
        let g = EvmGrinder;
        // ~268M expected candidates = dozens of launches, so the launch offset math is exercised.
        let pat = g.build_pattern(Some("abcdef1"), None, None, false).unwrap();
        let cancel = CancelToken::new();
        let (result, _) = grind(g, &[pat], &cancel, |_, _, _| {}).expect("gpu grind");
        assert!(
            result.attempts > EVM_PER_LAUNCH,
            "needs more than one launch to prove offset math"
        );
        assert!(result
            .keypair
            .address
            .to_ascii_lowercase()
            .starts_with("0xabcdef1"));
    }

    #[test]
    fn evm_benchmark_reports_real_throughput() {
        if !require_gpu() {
            return;
        }
        let (rate, _) = benchmark("evm", 1.0).expect("bench");
        assert!(rate > 1_000_000.0, "implausibly slow: {rate}");
    }

    #[test]
    fn sol_finds_short_prefix() {
        if !require_gpu() {
            return;
        }
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
