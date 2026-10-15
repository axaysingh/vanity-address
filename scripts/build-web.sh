#!/usr/bin/env bash
# Build the browser WASM engine for the landing site's web generator (landing/generate/).
# Cloudflare Pages serves landing/ as-is from Git, so the output is committed.
set -euo pipefail

cd "$(dirname "$0")/.."

# Homebrew's rust has no wasm32 target; prefer the rustup toolchain when installed.
if [[ -x "$HOME/.cargo/bin/rustup" ]]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi
if command -v brew >/dev/null && [[ -d "$(brew --prefix llvm 2>/dev/null)" ]]; then
  LLVM="$(brew --prefix llvm)"
  export CC_wasm32_unknown_unknown="${CC_wasm32_unknown_unknown:-$LLVM/bin/clang}"
  export AR_wasm32_unknown_unknown="${AR_wasm32_unknown_unknown:-$LLVM/bin/llvm-ar}"
fi
if command -v rustup >/dev/null && ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
  rustup target add wasm32-unknown-unknown
fi

OUT_DIR="landing/generate/wasm"
rm -rf "$OUT_DIR"

echo "Building vanity-wasm (web target)..."
wasm-pack build vanity-wasm --target web --out-dir "../$OUT_DIR" --out-name vanity_wasm --release --no-pack

rm -f "$OUT_DIR/package.json" "$OUT_DIR/.gitignore" "$OUT_DIR/README.md" "$OUT_DIR"/*.d.ts

echo "Done. Artifacts in $OUT_DIR"
