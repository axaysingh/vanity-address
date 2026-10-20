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
| **OpenCL GPU** | **EVM** + **Solana** full grind | Every GPU candidate is **re-derived on CPU** before export. |

## Reference numbers (maintainer laptop)

> Hardware: Apple M4 · OpenCL 1.2 · vanity-address **0.7.0** in-tree · 2026-10-20

| Mode | Chain | Pattern | ~keys/s | Notes |
| ---- | ----- | ------- | ------- | ----- |
| OpenCL GPU | EVM | short prefix | **re-bench after 0.7.0** | Correct affine secp256k1 + Keccak; slower than early broken builds that falsely “matched”. Not incremental EC. |
| OpenCL GPU | Solana | prefix `A` | **~35K** (M4 probe) | Full ed25519 + Base58 on device; CPU re-verify. **May be slower than CPU** on this silicon — do not treat as a speedup without your own CPU baseline. |
| CPU (fast) | EVM / Solana | short prefix | machine-dependent | Use HUD after calibration |

Treat these as order-of-magnitude only. Faster EVM GPU (Jacobian / incremental EC) is future work once vectors stay green.

## Reproducing

```bash
./scripts/bench.sh --json > bench-out.json
```

Include OS, CPU/GPU model, and binary version when sharing results in issues.
