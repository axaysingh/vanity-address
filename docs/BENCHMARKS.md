# Benchmarks

Public CPU / OpenCL numbers for vanity-address. Re-run on your machine with the scripts below — absolute keys/s depends heavily on silicon and pattern.

## Quick commands

```bash
# CPU warm-up (default grind path)
vanity-address --chain evm --prefix a --no-benchmark --force   # look at keys/s in the HUD
# or interactive menu → any short pattern

# OpenCL device list + GPU probe (needs `opencl` feature — default for CLI/desktop)
vanity-address gpu devices
vanity-address gpu bench --chain evm --seconds 2
vanity-address gpu bench --chain sol --seconds 2

# Scripted JSON (CI-friendly)
./scripts/bench.sh
```

## What we measure

| Path | Chains | Notes |
| ---- | ------ | ----- |
| **CPU (rayon)** | all menu chains | Default. Presets `eco` / `balanced` / `fast` / `extreme` change worker count. |
| **OpenCL GPU** | **EVM** (incremental EC + batch inversion) · **Solana** (experimental) | Optional CLI/desktop path. Every candidate is **re-derived on CPU**. `--gpu auto` uses the GPU for EVM only; Solana needs `--gpu require` and is currently slower than CPU. |

## Reference numbers (maintainer laptop)

> Hardware: Apple M4 · OpenCL 1.2 · vanity-address **0.7.0** in-tree · 2026-10-20

| Mode | Chain | Pattern | ~keys/s | Notes |
| ---- | ----- | ------- | ------- | ----- |
| OpenCL GPU | EVM | `gpu bench` (never-matching pattern) | **~46M** | Each work-item keeps a start point on the device and steps `+G` with one shared field inversion per 64 points. Measured: `0xabcdef1` prefix (~268M expected) in ~3 s. |
| OpenCL GPU | Solana | prefix `A` | **~35K** (M4 probe) | Full ed25519 + Base58 on device; CPU re-verify. **May be slower than CPU** on this silicon — do not treat as a speedup without your own CPU baseline. |
| CPU (fast) | EVM | 6–7 char prefix | **~0.5M** | `0xabcdef` (~17M expected) took 6–35 s; `0xabcdef1` did not finish in 200 s. GPU is roughly **90×** faster on this machine. |
| CPU (fast) | Solana | exact `abcd` | **~0.17M** | 11.3M expected in 68 s. The Solana GPU kernel (~35K) is ~5× slower, so it is not used by `--gpu auto`. |

Treat these as order-of-magnitude only; only Apple M4 was tested. NVIDIA / AMD results are untested — please share numbers. Solana GPU needs an on-device ed25519 fixed-base speedup before it beats CPU.

## Reproducing

```bash
./scripts/bench.sh --json > bench-out.json
```

Include OS, CPU/GPU model, and binary version when sharing results in issues.
