mod banner;
mod json_output;
mod menu;
mod terminal;
mod warnings;

use clap::{Parser, Subcommand};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use json_output::{print_error_json, print_success_json, ErrorCode};
use menu::run as run_interactive;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use vanity_core::{
    benchmark, benchmark_gpu, build_pattern_list, combine_split_keys, expected_attempts_any,
    format_attempts, format_duration, generate_split_keypair, gpu_available, gpu_supports_chain,
    grind_estimate, grind_gpu, grind_n, grind_patterns, list_gpu_devices, open_private_append,
    parse_secret_key, patterns_description, verify_address, BitcoinLikeGrinder, CancelToken, Chain,
    ChainGrinder, Create2Grinder, GpuMode, GrindPreset, GrindResult, Pattern, PatternRisk,
    PuzzleGrinder, SplitKeyGrinder, SystemProfile,
};

const BENCHMARK_SECS: f64 = 2.0;
const DEFAULT_RESULTS_FILE: &str = "vanity-results.txt";

#[derive(Parser)]
#[command(
    name = "vanity-address",
    version,
    author,
    about = "Fast, local multi-chain vanity address generator",
    long_about = "Generate cryptocurrency keypairs whose public address matches a prefix, \
                  suffix, and/or contains pattern. All keys are generated locally.\n\n\
                  Run without flags for the interactive menu."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Blockchain id or alias (e.g. sol, evm, base, hyperliquid, btc-segwit, sei) or deploy mode (evm-contract, trx-contract, sol-mint)
    #[arg(long)]
    chain: Option<String>,

    #[arg(long, help = "Address must start with this (supports * and comma OR)")]
    prefix: Option<String>,

    #[arg(long, help = "Address must end with this (supports * and comma OR)")]
    suffix: Option<String>,

    /// Substring anywhere in the address (supports `*` wildcards)
    #[arg(long)]
    contains: Option<String>,

    #[arg(long, help = "Exact casing (base58 chains only)")]
    exact: bool,

    /// Stop after finding N matches (default 1)
    #[arg(long, default_value_t = 1)]
    count: usize,

    #[arg(short, long)]
    quiet: bool,

    #[arg(long)]
    threads: Option<usize>,

    /// Grind power: eco | balanced | fast | extreme (aliases: efficient, best, superman)
    #[arg(long, value_name = "PRESET")]
    preset: Option<String>,

    /// Append match to vanity-results.txt (private keys — keep this file safe)
    #[arg(long)]
    save: bool,

    /// File path for saved keys (used with --save or interactive save prompt)
    #[arg(long, value_name = "PATH")]
    output: Option<String>,

    /// Skip the 2-second speed calibration warm-up before grinding
    #[arg(long)]
    no_benchmark: bool,

    /// Allow impractical patterns in CLI mode (years+ estimated grind time)
    #[arg(long)]
    force: bool,

    /// Machine-readable JSON on stdout (implies direct mode; use with --chain and pattern)
    #[arg(long)]
    json: bool,

    /// EVM CREATE2 salt grind (requires --deployer and --init-code-hash)
    #[arg(long)]
    create2: bool,

    /// CREATE2 deployer address (20-byte hex)
    #[arg(long, value_name = "HEX")]
    deployer: Option<String>,

    /// CREATE2 init code hash (32-byte hex)
    #[arg(long, value_name = "HEX")]
    init_code_hash: Option<String>,

    /// Bitcoin puzzle mode: free low bits of the private key (1..=256). Implies --chain btc.
    #[arg(long, value_name = "BITS")]
    puzzle: Option<u32>,

    /// Puzzle key high-bit template (32-byte hex). Default: all zeros. Requires --puzzle.
    #[arg(long, value_name = "HEX")]
    puzzle_start: Option<String>,

    /// Split-key grind: Alice's uncompressed/compressed secp256k1 pubkey hex (Bitcoin-family P2PKH).
    /// You only search a key *part* — the owner combines offline with `split-key combine`.
    #[arg(short = 'P', long = "split-pubkey", value_name = "HEX")]
    split_pubkey: Option<String>,

    /// OpenCL GPU grind: `auto` (prefer GPU for sol/evm, CPU fallback), `require`, or `off`
    #[arg(long, value_name = "MODE", default_value = "off")]
    gpu: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Verify that a private key (or CREATE2 salt) derives an address
    Verify {
        #[arg(long)]
        chain: String,
        #[arg(long)]
        address: String,
        #[arg(long)]
        key: String,
    },
    /// Split-key vanity (Bitcoin-family P2PKH): gen → grind -P → combine
    #[command(name = "split-key")]
    SplitKey {
        #[command(subcommand)]
        cmd: SplitKeyCmd,
    },
    /// List OpenCL devices / run a short GPU benchmark
    Gpu {
        #[command(subcommand)]
        cmd: GpuCmd,
    },
}

#[derive(Subcommand)]
enum GpuCmd {
    /// List OpenCL GPU devices
    Devices,
    /// Short OpenCL throughput probe (`sol` or `evm`)
    Bench {
        #[arg(long, default_value = "evm")]
        chain: String,
        #[arg(long, default_value_t = 2.0)]
        seconds: f64,
    },
}

#[derive(Subcommand)]
enum SplitKeyCmd {
    /// Generate Alice's half — keep the private key; share only the public key
    Gen {
        /// Bitcoin-family coin id (btc, ltc, doge, dash, rvn, …)
        #[arg(long, default_value = "btc")]
        chain: String,
    },
    /// Combine Alice's secret + Bob's part into the final spending key
    Combine {
        /// Alice private key (hex or WIF)
        #[arg(long)]
        alice: String,
        /// Bob's PrivkeyPart (hex or WIF) from the grind
        #[arg(long)]
        part: String,
        /// Bitcoin-family coin id (must match the grind)
        #[arg(long, default_value = "btc")]
        chain: String,
    },
}

struct RunConfig {
    chain: Chain,
    prefix: Option<String>,
    suffix: Option<String>,
    contains: Option<String>,
    exact: bool,
    count: usize,
    quiet: bool,
    threads: Option<usize>,
    /// Named power profile; ignored when `threads` is set.
    preset: Option<GrindPreset>,
    /// Skip the big system dump (interactive already showed a summary).
    compact_header: bool,
    /// Auto-save without prompting (CLI --save).
    save: bool,
    /// After grind, ask whether to save (interactive).
    prompt_save: bool,
    output: Option<PathBuf>,
    no_benchmark: bool,
    /// Already measured (interactive summary) — skip the warm-up.
    measured_keys_per_sec: Option<f64>,
    force: bool,
    json: bool,
    gpu: GpuMode,
}

impl Clone for RunConfig {
    fn clone(&self) -> Self {
        Self {
            chain: self.chain.clone(),
            prefix: self.prefix.clone(),
            suffix: self.suffix.clone(),
            contains: self.contains.clone(),
            exact: self.exact,
            count: self.count,
            quiet: self.quiet,
            threads: self.threads,
            preset: self.preset,
            compact_header: self.compact_header,
            save: self.save,
            prompt_save: self.prompt_save,
            output: self.output.clone(),
            no_benchmark: self.no_benchmark,
            measured_keys_per_sec: self.measured_keys_per_sec,
            force: self.force,
            json: self.json,
            gpu: self.gpu,
        }
    }
}

impl Cli {
    fn uses_direct_mode(&self) -> bool {
        self.command.is_some()
            || self.chain.is_some()
            || self.prefix.is_some()
            || self.suffix.is_some()
            || self.contains.is_some()
            || self.exact
            || self.count != 1
            || self.quiet
            || self.threads.is_some()
            || self.preset.is_some()
            || self.save
            || self.output.is_some()
            || self.no_benchmark
            || self.force
            || self.json
            || self.create2
            || self.puzzle.is_some()
            || self.puzzle_start.is_some()
            || self.split_pubkey.is_some()
            || self.gpu != "off"
    }

    fn into_run_config(self) -> Result<RunConfig, String> {
        if self.json && self.prefix.is_none() && self.suffix.is_none() && self.contains.is_none() {
            return Err("--json requires --prefix, --suffix, and/or --contains".into());
        }
        if self.puzzle_start.is_some() && self.puzzle.is_none() {
            return Err("--puzzle-start requires --puzzle".into());
        }

        let modes = [
            self.create2,
            self.puzzle.is_some(),
            self.split_pubkey.is_some(),
        ]
        .into_iter()
        .filter(|m| *m)
        .count();
        if modes > 1 {
            return Err("--create2, --puzzle, and --split-pubkey are mutually exclusive".into());
        }

        let chain = if self.create2 {
            let deployer = self
                .deployer
                .as_deref()
                .ok_or("--create2 requires --deployer")?;
            let init_hash = self
                .init_code_hash
                .as_deref()
                .ok_or("--create2 requires --init-code-hash")?;
            Chain::Create2(Create2Grinder::parse(deployer, init_hash)?)
        } else if let Some(bits) = self.puzzle {
            if let Some(ref id) = self.chain {
                let id = id.to_ascii_lowercase();
                if id != "btc" && id != "bitcoin" {
                    return Err("--puzzle currently supports --chain btc only".into());
                }
            }
            let start = match self.puzzle_start.as_deref() {
                Some(h) => PuzzleGrinder::parse_start(h)?,
                None => [0u8; 32],
            };
            Chain::Puzzle(PuzzleGrinder::new(
                bits,
                start,
                BitcoinLikeGrinder::bitcoin(),
            )?)
        } else if let Some(ref pk) = self.split_pubkey {
            let coin_id = self.chain.as_deref().unwrap_or("btc");
            let coin = BitcoinLikeGrinder::by_id(coin_id).ok_or_else(|| {
                format!(
                    "--split-pubkey needs a Bitcoin-family --chain (btc, ltc, doge, dash, rvn, …); got '{coin_id}'"
                )
            })?;
            Chain::SplitKey(SplitKeyGrinder::new(pk, coin)?)
        } else {
            match self.chain {
                Some(id) => Chain::from_id(&id)?,
                None => Chain::Solana(Default::default()),
            }
        };

        let preset = match self.preset.as_deref() {
            Some(s) => Some(GrindPreset::parse(s)?),
            None => None,
        };
        let gpu = GpuMode::parse(&self.gpu)?;

        Ok(RunConfig {
            chain,
            prefix: self.prefix,
            suffix: self.suffix,
            contains: self.contains,
            exact: self.exact,
            count: self.count.max(1),
            quiet: self.quiet,
            threads: self.threads,
            preset,
            compact_header: false,
            save: self.save,
            prompt_save: false,
            output: self.output.map(PathBuf::from),
            no_benchmark: self.no_benchmark,
            measured_keys_per_sec: None,
            force: self.force,
            json: self.json,
            gpu,
        })
    }
}

/// Grinding has no memory: after any number of misses the average wait is unchanged,
/// so show progress against the average instead of a countdown that can hit zero.
fn progress_line(attempts: u64, rate: f64, expected: f64) -> String {
    let share = attempts as f64 / expected.max(1.0) * 100.0;
    format!(
        "{} keys | {} keys/s | {:.0}% of average · avg {}",
        format_attempts(attempts as f64).cyan(),
        format_speed(rate).green(),
        share,
        format_duration(expected / rate.max(1.0))
    )
}

fn format_speed(n: f64) -> String {
    if n >= 1_000_000.0 {
        format!("{:.1}M", n / 1_000_000.0)
    } else if n >= 1_000.0 {
        format!("{:.0}K", n / 1_000.0)
    } else {
        format!("{:.0}", n)
    }
}

fn print_error(msg: &str) {
    eprintln!("{} {}", "error:".red().bold(), msg);
}

fn exit_error(msg: &str, code: ErrorCode, json: bool) -> ! {
    if json {
        let _ = print_error_json(msg, code);
    } else {
        print_error(msg);
    }
    std::process::exit(1);
}

fn run_grind(config: RunConfig) {
    let chain = config.chain;
    let json = config.json;

    if config.exact && !chain.supports_exact_case() {
        exit_error(
            &format!(
                "--exact is not supported for {} (chain: {})",
                chain.display_name(),
                chain.id()
            ),
            ErrorCode::ExactNotSupported,
            json,
        );
    }

    let patterns = match build_pattern_list(
        &chain,
        config.prefix.as_deref(),
        config.suffix.as_deref(),
        config.contains.as_deref(),
        config.exact,
    ) {
        Ok(p) => p,
        Err(e) => exit_error(&e, ErrorCode::InvalidPattern, json),
    };

    let expected = expected_attempts_any(
        &patterns
            .iter()
            .map(|p| chain.expected_attempts(p))
            .collect::<Vec<_>>(),
    );
    // Representative pattern for display / warnings (first alternative).
    let pattern = patterns[0].clone();
    let pattern_desc = patterns_description(&patterns);

    let mut profile = SystemProfile::detect();
    if let Some(preset) = config.preset {
        profile = profile.with_preset(preset);
    }
    // Explicit --threads wins over --preset.
    if let Some(threads) = config.threads {
        if threads == 0 {
            exit_error(
                "--threads must be at least 1",
                ErrorCode::InvalidThreads,
                json,
            );
        }
        profile = profile.with_threads(threads);
    }

    let interactive_out = !config.quiet && !json;
    let mut measured_keys_per_sec: Option<f64> = None;
    let mut benchmarked_now = false;

    if let Some(rate) = config.measured_keys_per_sec {
        measured_keys_per_sec = Some(rate);
        profile = profile.with_benchmark(rate);
    } else if !config.no_benchmark {
        if interactive_out && !config.compact_header {
            println!(
                "  {}  {}",
                "Calibrating".dimmed(),
                format!("{BENCHMARK_SECS:.0}s warm-up to measure real speed…").dimmed()
            );
        }
        match benchmark(chain.clone(), &profile, BENCHMARK_SECS) {
            Ok(rate) => {
                measured_keys_per_sec = Some(rate);
                profile = profile.with_benchmark(rate);
                benchmarked_now = true;
            }
            Err(e) => {
                if interactive_out && !config.compact_header {
                    eprintln!("  {}  benchmark skipped ({e})", "warn:".yellow().bold(),);
                }
            }
        }
    }

    let estimate = grind_estimate(
        expected,
        profile.estimated_keys_per_sec(chain.id()),
        &pattern,
    );

    // CLI direct mode: warn and block impractical patterns unless --force.
    if !config.compact_header && interactive_out {
        warnings::print_pattern_warnings(&estimate);
    }
    if (json || (interactive_out && !config.compact_header))
        && estimate.risk == PatternRisk::Impractical
        && !config.force
    {
        exit_error(
            "Pattern is not practical on a single machine. Shorten it or pass --force to grind anyway.",
            ErrorCode::ImpracticalPattern,
            json,
        );
    }

    if interactive_out && benchmarked_now {
        let rate = measured_keys_per_sec.unwrap_or_default();
        if !config.compact_header {
            println!(
                "  {}  {}",
                "Measured".dimmed(),
                format!("~{} keys/sec", format_speed(rate)).green()
            );
            println!(
                "  {}  {}",
                "Est. time".dimmed(),
                estimate.time_label.yellow()
            );
            println!();
        } else {
            println!(
                "  {}  {}",
                "Speed".dimmed(),
                format!("~{} keys/sec (measured)", format_speed(rate)).green()
            );
        }
    }

    if !config.quiet && !json {
        if config.compact_header {
            println!();
            println!(
                "  {}  {} · {}",
                "Grinding".cyan().bold(),
                chain.display_name().bold(),
                pattern_desc.dimmed()
            );
            println!("  {}", "(Ctrl+C to stop)".dimmed());
            println!();
        } else {
            println!();
            println!("  {}  {}", "System".dimmed(), profile.summary_line().cyan());
            println!("  {}  {}", "CPU".dimmed(), profile.cpu_description());
            println!(
                "  {}  {}",
                "Workers".dimmed(),
                profile.worker_description().green()
            );
            println!("  {}  {}", "Chain".dimmed(), chain.display_name().bold());
            println!("  {}  {}", "Target".dimmed(), pattern_desc.bold());
            println!("  {}  {}", "Mode".dimmed(), pattern.case_mode());
            if config.count > 1 {
                println!(
                    "  {}  {}",
                    "Count".dimmed(),
                    config.count.to_string().cyan()
                );
            }
            println!(
                "  {}  ~{} attempts (average)",
                "Expected".dimmed(),
                format_attempts(expected).yellow()
            );
            println!("  {}  {}", "Hint".dimmed(), chain.pattern_hint().dimmed());
            println!();
            println!("{}", "Grinding...  (Ctrl+C to stop)".dimmed());
            println!();
        }
    }

    let pb = if config.quiet || json {
        None
    } else {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap(),
        );
        bar.enable_steady_tick(std::time::Duration::from_millis(80));
        Some(bar)
    };

    let want_gpu = match config.gpu {
        GpuMode::Off => false,
        GpuMode::Auto | GpuMode::Require => gpu_supports_chain(chain.id()) && (config.count <= 1),
    };
    if want_gpu && interactive_out {
        let devices = list_gpu_devices();
        if let Some(d) = devices.first() {
            println!(
                "  {}  {} ({})",
                "GPU".dimmed(),
                d.name.cyan(),
                match config.gpu {
                    GpuMode::Require => "required",
                    _ => "auto",
                }
            );
        } else if config.gpu == GpuMode::Require {
            exit_error(
                "No OpenCL GPU found (--gpu require). Install drivers or use --gpu off.",
                ErrorCode::GrindFailed,
                json,
            );
        }
    }

    let results = if want_gpu {
        match grind_gpu(
            chain.clone(),
            &patterns,
            &profile,
            &CancelToken::new(),
            |attempts, rate, _eta_min| {
                if let Some(ref bar) = pb {
                    bar.set_message(format!("gpu · {}", progress_line(attempts, rate, expected)));
                }
            },
        ) {
            Ok((r, report)) => {
                if interactive_out {
                    println!(
                        "  {}  {} · ~{} keys/s",
                        "OpenCL".dimmed(),
                        report.device_name.cyan(),
                        format_speed(report.attempts as f64 / report.elapsed_secs.max(0.001))
                    );
                }
                vec![r]
            }
            Err(e) if config.gpu == GpuMode::Require => {
                exit_error(&e, ErrorCode::GrindFailed, json)
            }
            Err(e) => {
                if interactive_out {
                    println!(
                        "  {}  GPU unavailable ({e}) — falling back to CPU",
                        "warn:".yellow().bold()
                    );
                }
                match grind_patterns(
                    chain.clone(),
                    &patterns,
                    &profile,
                    &CancelToken::new(),
                    |attempts, rate, _eta_min| {
                        if let Some(ref bar) = pb {
                            bar.set_message(progress_line(attempts, rate, expected));
                        }
                    },
                ) {
                    Ok(r) => vec![r],
                    Err(e) => exit_error(&e, ErrorCode::GrindFailed, json),
                }
            }
        }
    } else if config.count <= 1 {
        match grind_patterns(
            chain.clone(),
            &patterns,
            &profile,
            &CancelToken::new(),
            |attempts, rate, _eta_min| {
                if let Some(ref bar) = pb {
                    bar.set_message(progress_line(attempts, rate, expected));
                }
            },
        ) {
            Ok(r) => vec![r],
            Err(e) => exit_error(&e, ErrorCode::GrindFailed, json),
        }
    } else {
        match grind_n(
            chain.clone(),
            &patterns,
            config.count,
            &profile,
            &CancelToken::new(),
            |attempts, rate, _eta_min, which| {
                if let Some(ref bar) = pb {
                    bar.set_message(format!(
                        "match {}/{} · {}",
                        which,
                        config.count,
                        progress_line(attempts, rate, expected)
                    ));
                }
            },
        ) {
            Ok(r) => r,
            Err(e) => exit_error(&e, ErrorCode::GrindFailed, json),
        }
    };

    if let Some(ref bar) = pb {
        bar.finish_and_clear();
    }

    for (idx, result) in results.iter().enumerate() {
        if json {
            let output_path = config
                .output
                .clone()
                .unwrap_or_else(|| PathBuf::from(DEFAULT_RESULTS_FILE));
            let mut saved_to: Option<String> = None;
            if config.save {
                match save_result(&output_path, &chain, &pattern, result) {
                    Ok(path) => saved_to = Some(path),
                    Err(e) => exit_error(
                        &format!("could not save results: {e}"),
                        ErrorCode::IoError,
                        json,
                    ),
                }
            }
            if let Err(e) = print_success_json(
                chain.id(),
                chain.display_name(),
                &pattern,
                result,
                measured_keys_per_sec,
                saved_to.as_deref(),
            ) {
                exit_error(
                    &format!("could not serialize JSON: {e}"),
                    ErrorCode::IoError,
                    json,
                );
            }
            continue;
        }

        if config.quiet {
            println!("{}", result.keypair.address);
            for export in &result.keypair.exports {
                println!("{}: {}", export.label, export.value);
            }
        } else {
            if results.len() > 1 {
                println!();
                println!("  {}  {}/{}", "Match".cyan().bold(), idx + 1, results.len());
            }
            print_success(result, &pattern);
        }

        let output_path = config
            .output
            .clone()
            .unwrap_or_else(|| PathBuf::from(DEFAULT_RESULTS_FILE));
        let save_prompt = format!("  Save keys to {}? [y/N]: ", output_path.display());

        let should_save = if config.save {
            if !config.quiet && !json && idx == 0 {
                println!();
                println!(
                    "  {}",
                    "--save writes PLAINTEXT private keys to disk. Encrypt, keep offline, never commit."
                        .red()
                        .bold()
                );
            }
            true
        } else if config.prompt_save && !config.quiet && idx == results.len() - 1 {
            println!();
            println!(
                "  {}",
                "Saved files are plaintext keys — anyone with the file can drain the wallet."
                    .yellow()
            );
            terminal::read_yes_no_key(&save_prompt, false, false).unwrap_or(false)
        } else {
            false
        };

        if should_save {
            match save_result(&output_path, &chain, &pattern, result) {
                Ok(path) => {
                    if !config.quiet {
                        println!("  {}  {}", "Saved →".green().bold(), path.cyan());
                        println!(
                            "  {}",
                            "PLAINTEXT private keys — do not commit, email, or cloud-sync this file."
                                .red()
                                .bold()
                        );
                    }
                }
                Err(e) => print_error(&format!("could not save results: {e}")),
            }
        }
    }

    let _ = io::stdout().flush();
}

fn print_success(result: &GrindResult, pattern: &Pattern) {
    let rate = if result.elapsed_secs > 0.0 {
        result.attempts as f64 / result.elapsed_secs
    } else {
        0.0
    };

    println!();
    println!("  {}", "✓ MATCH FOUND".black().on_green().bold());
    println!();

    // ── Public address ──────────────────────────────────────────
    println!("  {}", "── Public Address ──".bold().cyan());
    println!();
    println!("  {}", highlight_address(&result.keypair.address, pattern));
    println!();
    println!(
        "  {:<12} {}",
        "Found in".dimmed(),
        format!("{:.2}s", result.elapsed_secs).green()
    );
    println!(
        "  {:<12} {}  ({:.0} keys/s)",
        "Attempts".dimmed(),
        format_number(result.attempts).yellow(),
        rate
    );
    println!();

    // ── Private keys ────────────────────────────────────────────
    println!("  {}", "── Private Keys ──".bold().red());
    println!(
        "  {}",
        "⚠  Never share these. Anyone with them can drain the wallet."
            .red()
            .dimmed()
    );
    println!();

    let label_width = result
        .keypair
        .exports
        .iter()
        .map(|e| e.label.len())
        .max()
        .unwrap_or(12)
        .max(12);

    for export in &result.keypair.exports {
        println!(
            "  {:<width$}  {}",
            export.label.dimmed(),
            export.value.bold().white(),
            width = label_width
        );
        if let Some(ref hint) = export.hint {
            println!("  {:width$}  {}", "", hint.dimmed(), width = label_width);
        }
        println!();
    }

    // ── Copy-friendly plain block ───────────────────────────────
    println!("  {}", "── Copy / Paste ──".bold().dimmed());
    println!();
    println!("  Address: {}", result.keypair.address);
    for export in &result.keypair.exports {
        println!("  {}: {}", export.label, export.value);
    }
    println!();

    println!(
        "  {}",
        "Verify the address in your wallet before sending funds."
            .yellow()
            .dimmed()
    );
}

/// Render address with matched prefix/suffix/contains highlighted in green.
fn highlight_address(address: &str, pattern: &Pattern) -> String {
    let ignore_case = pattern.ignore_case;
    let prefix = &pattern.prefix_match;
    let suffix = &pattern.suffix_match;
    let contains = &pattern.contains_match;

    let starts = !prefix.is_empty()
        && address.len() >= prefix.len()
        && if ignore_case {
            address[..prefix.len()].eq_ignore_ascii_case(prefix)
        } else {
            address.starts_with(prefix.as_str())
        };

    let ends = !suffix.is_empty()
        && address.len() >= suffix.len()
        && if ignore_case {
            address[address.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
        } else {
            address.ends_with(suffix.as_str())
        };

    if starts || ends {
        let mid_start = if starts { prefix.len() } else { 0 };
        let mid_end = if ends {
            address.len() - suffix.len()
        } else {
            address.len()
        };
        let mid_end = mid_end.max(mid_start);

        let mut out = String::new();
        if starts {
            out.push_str(&address[..mid_start].green().bold().to_string());
        }
        if mid_start < mid_end {
            out.push_str(&address[mid_start..mid_end].bold().white().to_string());
        }
        if ends {
            out.push_str(&address[mid_end..].green().bold().to_string());
        }
        return out;
    }

    // Contains: highlight first literal segment (ignore wildcards for display)
    let needle = if contains.contains('*') {
        contains
            .split('*')
            .find(|p| !p.is_empty())
            .unwrap_or("")
            .to_string()
    } else {
        contains.clone()
    };
    if !needle.is_empty() {
        let hay = if ignore_case {
            address.to_ascii_lowercase()
        } else {
            address.to_string()
        };
        let n = if ignore_case {
            needle.to_ascii_lowercase()
        } else {
            needle.clone()
        };
        if let Some(idx) = hay.find(&n) {
            let end = idx + needle.len();
            return format!(
                "{}{}{}",
                address[..idx].bold().white(),
                address[idx..end].green().bold(),
                address[end..].bold().white()
            );
        }
    }

    address.bold().white().to_string()
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

fn save_result(
    path: &Path,
    chain: &Chain,
    pattern: &Pattern,
    result: &GrindResult,
) -> io::Result<String> {
    let mut file = open_private_append(path)?;

    let timestamp = chrono_timestamp();
    writeln!(file, "━")?;
    writeln!(file, "vanity-address match · {timestamp}")?;
    writeln!(file, "chain:   {}", chain.display_name())?;
    writeln!(file, "target:  {}", pattern.description())?;
    writeln!(file, "mode:    {}", pattern.case_mode())?;
    writeln!(
        file,
        "stats:   {} attempts in {:.2}s",
        result.attempts, result.elapsed_secs
    )?;
    writeln!(file)?;
    writeln!(file, "Address: {}", result.keypair.address)?;
    for export in &result.keypair.exports {
        writeln!(file, "{}: {}", export.label, export.value)?;
    }
    writeln!(file)?;
    writeln!(
        file,
        "WARNING: PLAINTEXT PRIVATE KEYS. Anyone with this file can steal funds."
    )?;
    writeln!(
        file,
        "Keep offline, encrypt backups, never commit/email/cloud-sync, shred when done."
    )?;
    writeln!(
        file,
        "This tool is a key generator — not a wallet. Verify the address before sending value."
    )?;
    writeln!(file)?;

    Ok(path.display().to_string())
}

fn chrono_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Rough UTC calendar date without pulling in chrono.
    // Algorithm: Howard Hinnant days_from_civil inverse.
    let z = (secs / 86_400) as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    let tod = secs % 86_400;
    let h = tod / 3600;
    let min = (tod % 3600) / 60;
    let s = tod % 60;
    format!("{y:04}-{m:02}-{d:02} {h:02}:{min:02}:{s:02} UTC")
}

fn bitcoin_family_coin(id: &str) -> Result<BitcoinLikeGrinder, String> {
    BitcoinLikeGrinder::by_id(id).ok_or_else(|| {
        format!("Unknown Bitcoin-family chain '{id}'. Use btc, ltc, doge, dash, rvn, …")
    })
}

fn run_split_key(cmd: SplitKeyCmd, json: bool) {
    match cmd {
        SplitKeyCmd::Gen { chain } => match bitcoin_family_coin(&chain) {
            Ok(coin) => {
                let kp = generate_split_keypair(&coin);
                if json {
                    println!(
                        "{{\"ok\":true,\"chain\":\"{}\",\"address\":\"{}\",\"private_key_hex\":\"{}\",\"private_key_wif\":\"{}\",\"public_key_hex\":\"{}\"}}",
                        coin.id, kp.address, kp.private_key_hex, kp.private_key_wif, kp.public_key_hex
                    );
                } else {
                    println!();
                    println!(
                        "  {}  {}",
                        "Split-key (Alice)".green().bold(),
                        coin.display_name
                    );
                    println!();
                    println!("  {}", "KEEP SECRET — never share these:".red().bold());
                    println!("  Private (hex): {}", kp.private_key_hex);
                    println!("  Private (WIF): {}", kp.private_key_wif);
                    println!();
                    println!(
                        "  {}",
                        "SHARE with the grinder (safe to send):".yellow().bold()
                    );
                    println!("  Public key:    {}", kp.public_key_hex);
                    println!();
                    println!(
                        "  Next: vanity-address --chain {} -P <pubkey> --prefix …",
                        coin.id
                    );
                    println!(
                        "  Then: vanity-address split-key combine --alice <secret> --part <part>"
                    );
                    println!();
                }
            }
            Err(e) => exit_error(&e, ErrorCode::InvalidChain, json),
        },
        SplitKeyCmd::Combine { alice, part, chain } => match (|| -> Result<_, String> {
            let coin = bitcoin_family_coin(&chain)?;
            let alice_sk = parse_secret_key(&alice)?;
            let part_sk = parse_secret_key(&part)?;
            let kp = combine_split_keys(&alice_sk, &part_sk, &coin)?;
            Ok((coin, kp))
        })() {
            Ok((coin, kp)) => {
                if json {
                    println!(
                        "{{\"ok\":true,\"chain\":\"{}\",\"address\":\"{}\",\"private_key_hex\":\"{}\",\"private_key_wif\":\"{}\"}}",
                        coin.id, kp.address, kp.private_key_hex, kp.private_key_wif
                    );
                } else {
                    println!();
                    println!(
                        "  {}  {}",
                        "Split-key combined".green().bold(),
                        coin.display_name
                    );
                    println!();
                    println!("  Address:       {}", kp.address.cyan().bold());
                    println!("  Private (hex): {}", kp.private_key_hex);
                    println!("  Private (WIF): {}", kp.private_key_wif);
                    println!();
                    println!(
                        "  {}",
                        "FINAL spending key — store offline. This tool is not a wallet."
                            .red()
                            .bold()
                    );
                    println!();
                }
            }
            Err(e) => exit_error(&e, ErrorCode::InvalidArgs, json),
        },
    }
}

fn main() {
    terminal::install_ctrlc_handler();

    let cli = Cli::parse();
    let json = cli.json;

    if let Some(Commands::Verify {
        chain,
        address,
        key,
    }) = &cli.command
    {
        match verify_address(chain, address, key) {
            Ok(()) => {
                if json {
                    println!(
                        "{{\"ok\":true,\"chain\":\"{}\",\"address\":\"{}\"}}",
                        chain, address
                    );
                } else {
                    println!("  {}  key derives {}", "✓ verified".green().bold(), address);
                }
            }
            Err(e) => exit_error(&e, ErrorCode::InvalidArgs, json),
        }
        return;
    }

    if let Some(Commands::SplitKey { cmd }) = cli.command {
        run_split_key(cmd, json);
        return;
    }

    if let Some(Commands::Gpu { cmd }) = cli.command {
        match cmd {
            GpuCmd::Devices => {
                let devices = list_gpu_devices();
                if devices.is_empty() {
                    if json {
                        println!("{{\"ok\":true,\"devices\":[],\"available\":false}}");
                    } else {
                        println!(
                            "  {}  no OpenCL GPUs (built with opencl={})",
                            "GPU".yellow().bold(),
                            gpu_available()
                        );
                    }
                } else if json {
                    let list: Vec<_> = devices
                        .iter()
                        .map(|d| {
                            format!(
                                "{{\"index\":{},\"name\":\"{}\",\"vendor\":\"{}\",\"compute_units\":{},\"max_clock_mhz\":{}}}",
                                d.index,
                                d.name.replace('"', "\\\""),
                                d.vendor.replace('"', "\\\""),
                                d.compute_units,
                                d.max_clock_mhz
                            )
                        })
                        .collect();
                    println!(
                        "{{\"ok\":true,\"devices\":[{}],\"available\":true}}",
                        list.join(",")
                    );
                } else {
                    println!();
                    println!("  {}", "OpenCL devices".green().bold());
                    for d in &devices {
                        println!(
                            "  [{}] {} — {} · {} CU · {} MHz",
                            d.index, d.name, d.vendor, d.compute_units, d.max_clock_mhz
                        );
                    }
                    println!();
                }
            }
            GpuCmd::Bench { chain, seconds } => match benchmark_gpu(&chain, seconds) {
                Ok((rate, name)) => {
                    if json {
                        println!(
                            "{{\"ok\":true,\"chain\":\"{}\",\"device\":\"{}\",\"keys_per_sec\":{}}}",
                            chain, name, rate
                        );
                    } else {
                        println!();
                        println!("  {}  {}", "GPU bench".green().bold(), name);
                        println!("  chain:  {chain}");
                        println!("  speed:  ~{} keys/sec", format_speed(rate));
                        println!();
                    }
                }
                Err(e) => exit_error(&e, ErrorCode::GrindFailed, json),
            },
        }
        return;
    }

    if cli.uses_direct_mode() {
        match cli.into_run_config() {
            Ok(config) => run_grind(config),
            Err(e) => {
                let code = if e.contains("Unknown chain") {
                    ErrorCode::InvalidChain
                } else {
                    ErrorCode::InvalidArgs
                };
                exit_error(&e, code, json);
            }
        }
        return;
    }

    // Interactive old-school menu mode (default)
    loop {
        let Some(config) = run_interactive() else {
            return;
        };

        let grind_config = RunConfig {
            chain: config.chain.clone(),
            prefix: config.prefix.clone(),
            suffix: config.suffix.clone(),
            contains: config.contains.clone(),
            exact: config.exact,
            count: 1,
            quiet: false,
            threads: None,
            preset: Some(config.preset),
            compact_header: true,
            save: false,
            prompt_save: true,
            output: None,
            no_benchmark: false,
            measured_keys_per_sec: config.measured_keys_per_sec,
            force: false,
            json: false,
            gpu: GpuMode::Off,
        };

        loop {
            run_grind(grind_config.clone());

            println!();
            let generate_more =
                terminal::read_yes_no_key("  Generate another address? [y/N]: ", false, false)
                    .unwrap_or(false);

            if generate_more {
                continue;
            }
            // N → back to main menu (Start / Help / Exit)
            break;
        }
    }
}
