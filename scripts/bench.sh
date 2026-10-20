#!/usr/bin/env bash
# CPU + OpenCL probe for vanity-address. Requires a release CLI binary.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${VANITY_BIN:-$ROOT/target/release/vanity-address}"
JSON=0
[[ "${1:-}" == "--json" ]] && JSON=1

if [[ ! -x "$BIN" ]]; then
  echo "Building release CLI…" >&2
  cargo build -p vanity-address --release
fi

ts() { date -u +"%Y-%m-%dT%H:%M:%SZ"; }

if [[ "$JSON" -eq 1 ]]; then
  echo "{"
  echo "  \"timestamp\": \"$(ts)\","
  echo "  \"binary\": \"$BIN\","
  echo -n "  \"gpu_devices\": "
  "$BIN" --json gpu devices || echo '{"ok":false}'
  echo ","
  echo -n "  \"gpu_bench_evm\": "
  "$BIN" --json gpu bench --chain evm --seconds 2 || echo '{"ok":false}'
  echo ","
  echo -n "  \"gpu_bench_sol\": "
  "$BIN" --json gpu bench --chain sol --seconds 2 || echo '{"ok":false}'
  echo
  echo "}"
else
  echo "=== vanity-address bench ($(ts)) ==="
  echo "binary: $BIN"
  echo
  echo "--- OpenCL devices ---"
  "$BIN" gpu devices || true
  echo
  echo "--- GPU bench (evm, 2s) ---"
  "$BIN" gpu bench --chain evm --seconds 2 || true
  echo
  echo "--- GPU bench (sol, 2s) ---"
  "$BIN" gpu bench --chain sol --seconds 2 || true
  echo
  echo "See docs/BENCHMARKS.md"
fi
