use rayon::ThreadPoolBuilder;
use std::fmt;

/// Named grind power profiles — frontends pass an id; core owns the thread math.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GrindPreset {
    /// ~25% of cores — cool / battery-friendly.
    Eco,
    /// Current auto-tune (usually cores − 1). Default.
    #[default]
    Balanced,
    /// Use all logical CPUs (still respects memory caps).
    Fast,
    /// Same workers as Fast; denser progress reporting.
    Extreme,
}

impl GrindPreset {
    pub fn id(self) -> &'static str {
        match self {
            Self::Eco => "eco",
            Self::Balanced => "balanced",
            Self::Fast => "fast",
            Self::Extreme => "extreme",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Eco => "Eco",
            Self::Balanced => "Balanced",
            Self::Fast => "Fast",
            Self::Extreme => "Extreme",
        }
    }

    /// Parse a preset id or alias (`efficient`, `best`, `superman`, …).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "eco" | "minimal" | "efficient" | "low" => Ok(Self::Eco),
            "balanced" | "auto" | "default" | "moderate" => Ok(Self::Balanced),
            "fast" | "best" | "performance" | "high" => Ok(Self::Fast),
            "extreme" | "superman" | "max" | "all-out" | "allout" => Ok(Self::Extreme),
            other => Err(format!(
                "Unknown preset '{other}'. Use: eco, balanced, fast, extreme"
            )),
        }
    }

    pub const ALL: [GrindPreset; 4] = [Self::Eco, Self::Balanced, Self::Fast, Self::Extreme];
}

/// Detected host capabilities used to tune the grinder before work starts.
#[derive(Debug, Clone)]
pub struct SystemProfile {
    pub logical_cpus: usize,
    pub physical_cpus: Option<usize>,
    pub total_memory_mb: u64,
    pub available_memory_mb: u64,
    pub worker_threads: usize,
    pub progress_interval: u64,
    pub memory_pressure: MemoryPressure,
    /// Set after a live benchmark warm-up for accurate ETA.
    pub measured_keys_per_sec: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressure {
    Comfortable,
    Moderate,
    Low,
}

impl fmt::Display for MemoryPressure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryPressure::Comfortable => write!(f, "comfortable"),
            MemoryPressure::Moderate => write!(f, "moderate"),
            MemoryPressure::Low => write!(f, "low"),
        }
    }
}

impl SystemProfile {
    /// Probe the current machine and derive grinding settings.
    pub fn detect() -> Self {
        let logical_cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);

        let (physical_cpus, total_memory_mb, available_memory_mb) = probe_sysinfo();

        let memory_pressure = classify_memory_pressure(available_memory_mb);
        let worker_threads =
            compute_worker_threads(logical_cpus, physical_cpus, available_memory_mb);
        let progress_interval = compute_progress_interval(worker_threads);

        Self {
            logical_cpus,
            physical_cpus,
            total_memory_mb,
            available_memory_mb,
            worker_threads,
            progress_interval,
            memory_pressure,
            measured_keys_per_sec: None,
        }
    }

    /// Store a live benchmark result from `benchmark()`.
    pub fn with_benchmark(mut self, keys_per_sec: f64) -> Self {
        self.measured_keys_per_sec = Some(keys_per_sec.max(1.0));
        self
    }

    /// Override thread count (e.g. from `--threads` CLI flag).
    pub fn with_threads(mut self, threads: usize) -> Self {
        let threads = threads.clamp(1, self.logical_cpus);
        self.worker_threads = threads;
        self.progress_interval = compute_progress_interval(threads);
        self
    }

    /// Apply a named power preset. Call before `with_threads` if both are set
    /// (explicit thread count should win at the call site).
    pub fn with_preset(mut self, preset: GrindPreset) -> Self {
        let logical = self.logical_cpus.max(1);
        let mem_cap = memory_worker_cap(self.available_memory_mb);
        let threads = match preset {
            GrindPreset::Eco => (logical / 4).max(1).min(mem_cap),
            GrindPreset::Balanced => {
                compute_worker_threads(logical, self.physical_cpus, self.available_memory_mb)
            }
            GrindPreset::Fast | GrindPreset::Extreme => logical.min(mem_cap).max(1),
        };
        self.worker_threads = threads;
        let base = compute_progress_interval(threads);
        self.progress_interval = match preset {
            GrindPreset::Extreme => (base / 2).max(50_000),
            _ => base,
        };
        self
    }

    pub fn summary_line(&self) -> String {
        let cores = self.cpu_description();

        format!(
            "{cores} · {:.1} GB RAM ({:.1} GB free) · {} workers · memory: {}",
            self.total_memory_mb as f64 / 1024.0,
            self.available_memory_mb as f64 / 1024.0,
            self.worker_threads,
            self.memory_pressure,
        )
    }

    pub fn cpu_description(&self) -> String {
        match self.physical_cpus {
            Some(physical) if physical != self.logical_cpus => {
                format!("{} logical · {} physical", self.logical_cpus, physical)
            }
            Some(physical) => format!("{physical} cores"),
            None => format!("{} cores", self.logical_cpus),
        }
    }

    pub fn worker_description(&self) -> String {
        let reserved = self.logical_cpus.saturating_sub(self.worker_threads);
        if reserved > 0 {
            format!(
                "{} threads grinding · {} core{} reserved for OS",
                self.worker_threads,
                reserved,
                if reserved == 1 { "" } else { "s" }
            )
        } else {
            format!("{} threads (all cores)", self.worker_threads)
        }
    }

    pub fn estimated_keys_per_sec(&self, chain_id: &str) -> f64 {
        if let Some(measured) = self.measured_keys_per_sec {
            return measured;
        }
        let per_thread = match chain_id {
            "evm" | "evm-contract" | "trx-contract" | "create2" | "aptos" | "sui" | "near" => {
                35_000.0
            }
            _ => 80_000.0,
        };
        per_thread * self.worker_threads as f64
    }
}

fn probe_sysinfo() -> (Option<usize>, u64, u64) {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();

    let total_memory_mb = sys.total_memory() / 1024 / 1024;
    let mut available_memory_mb = sys.available_memory() / 1024 / 1024;

    // macOS often reports near-zero "available" RAM — use a sane fallback.
    if available_memory_mb < 256 && total_memory_mb > 512 {
        available_memory_mb = (total_memory_mb * 3) / 4;
    }

    let physical_cpus = sys.physical_core_count();

    (physical_cpus, total_memory_mb, available_memory_mb)
}

fn classify_memory_pressure(available_mb: u64) -> MemoryPressure {
    if available_mb < 512 {
        MemoryPressure::Low
    } else if available_mb < 2_048 {
        MemoryPressure::Moderate
    } else {
        MemoryPressure::Comfortable
    }
}

/// How many rayon workers to spawn for this host.
pub fn compute_worker_threads(
    logical_cpus: usize,
    physical_cpus: Option<usize>,
    available_mem_mb: u64,
) -> usize {
    let logical_cpus = logical_cpus.max(1);

    // Leave one core for the OS / terminal on machines with headroom.
    let mut threads = match logical_cpus {
        1 => 1,
        2 => 2,
        n => n - 1,
    };

    // Prefer not to oversubscribe physical cores when SMT is present.
    if let Some(physical) = physical_cpus {
        if physical > 0 && threads > physical {
            threads = physical;
        }
    }

    // Back off when memory is tight — each worker holds crypto state on its stack.
    threads.min(memory_worker_cap(available_mem_mb)).max(1)
}

fn memory_worker_cap(available_mem_mb: u64) -> usize {
    match available_mem_mb {
        m if m < 512 => 1,
        m if m < 1_024 => 2,
        m if m < 2_048 => 4,
        m if m < 4_096 => 8,
        _ => usize::MAX,
    }
}

/// Progress callback frequency — scales with worker count, bounded to limit overhead.
pub fn compute_progress_interval(worker_threads: usize) -> u64 {
    (50_000_u64 * worker_threads as u64).clamp(100_000, 2_000_000)
}

/// Build a rayon pool sized for this profile.
pub fn build_thread_pool(profile: &SystemProfile) -> Result<rayon::ThreadPool, String> {
    ThreadPoolBuilder::new()
        .num_threads(profile.worker_threads)
        .thread_name(|i| format!("vanity-worker-{i}"))
        .build()
        .map_err(|e| format!("Failed to configure thread pool: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_core_machine_uses_one_worker() {
        assert_eq!(compute_worker_threads(1, Some(1), 16_384), 1);
    }

    #[test]
    fn reserves_core_on_typical_laptop() {
        assert_eq!(compute_worker_threads(8, Some(8), 16_384), 7);
    }

    #[test]
    fn low_memory_caps_workers() {
        assert_eq!(compute_worker_threads(16, Some(16), 800), 2);
    }

    #[test]
    fn progress_interval_scales_with_threads() {
        assert_eq!(compute_progress_interval(1), 100_000);
        assert_eq!(compute_progress_interval(8), 400_000);
        assert_eq!(compute_progress_interval(64), 2_000_000);
    }

    fn profile(logical: usize, physical: Option<usize>, mem_mb: u64) -> SystemProfile {
        SystemProfile {
            logical_cpus: logical,
            physical_cpus: physical,
            total_memory_mb: mem_mb,
            available_memory_mb: mem_mb,
            worker_threads: 1,
            progress_interval: 100_000,
            memory_pressure: MemoryPressure::Comfortable,
            measured_keys_per_sec: None,
        }
    }

    #[test]
    fn eco_on_8_cores_uses_2() {
        let p = profile(8, Some(8), 16_384).with_preset(GrindPreset::Eco);
        assert_eq!(p.worker_threads, 2);
    }

    #[test]
    fn balanced_matches_compute_worker_threads() {
        let p = profile(8, Some(8), 16_384).with_preset(GrindPreset::Balanced);
        assert_eq!(p.worker_threads, compute_worker_threads(8, Some(8), 16_384));
    }

    #[test]
    fn fast_uses_all_logical_cpus() {
        let p = profile(8, Some(8), 16_384).with_preset(GrindPreset::Fast);
        assert_eq!(p.worker_threads, 8);
    }

    #[test]
    fn extreme_same_threads_as_fast_tighter_progress() {
        let fast = profile(8, Some(8), 16_384).with_preset(GrindPreset::Fast);
        let extreme = profile(8, Some(8), 16_384).with_preset(GrindPreset::Extreme);
        assert_eq!(extreme.worker_threads, fast.worker_threads);
        assert!(extreme.progress_interval < fast.progress_interval);
        assert_eq!(
            extreme.progress_interval,
            (compute_progress_interval(8) / 2).max(50_000)
        );
    }

    #[test]
    fn parse_accepts_aliases() {
        assert_eq!(GrindPreset::parse("efficient").unwrap(), GrindPreset::Eco);
        assert_eq!(GrindPreset::parse("best").unwrap(), GrindPreset::Fast);
        assert_eq!(
            GrindPreset::parse("performance").unwrap(),
            GrindPreset::Fast
        );
        assert_eq!(
            GrindPreset::parse("superman").unwrap(),
            GrindPreset::Extreme
        );
        assert!(GrindPreset::parse("nope").is_err());
    }

    #[test]
    fn mem_cap_still_limits_extreme() {
        let p = profile(16, Some(16), 800).with_preset(GrindPreset::Extreme);
        assert_eq!(p.worker_threads, 2);
    }

    #[test]
    fn eco_on_dual_core_still_one_worker_min() {
        let p = profile(2, Some(2), 16_384).with_preset(GrindPreset::Eco);
        assert_eq!(p.worker_threads, 1);
    }
}
