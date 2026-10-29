#!/usr/bin/env bash
# Publish platform packages then main vanity-address to npm.
# Usage:
#   ./scripts/prepare-npm.sh 0.3.5
#   ./scripts/publish-npm.sh            # dry-run first? use --dry-run
#   ./scripts/publish-npm.sh --dry-run
#   ./scripts/publish-npm.sh --main-only [--dry-run]   # docs/metadata patch: main package only
#
# Requires: binaries prepared via prepare-npm.sh. Runs `npm login` if needed.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DRY=""
MAIN_ONLY=""
for arg in "$@"; do
  case "${arg}" in
    --dry-run) DRY="--dry-run" ;;
    --main-only) MAIN_ONLY=1 ;;
  esac
done

PLATFORMS=(
  vanity-address-darwin-arm64
  vanity-address-darwin-x64
  vanity-address-linux-x64
  vanity-address-win32-x64
)

require_bin() {
  local dir="$1" name="$2"
  local path="${ROOT}/npm/${dir}/bin/${name}"
  if [[ ! -f "${path}" ]]; then
    echo "Missing ${path} — run ./scripts/prepare-npm.sh first" >&2
    exit 1
  fi
}

require_wasm() {
  local target="$1"
  local path="${ROOT}/npm/vanity-address/wasm/${target}/vanity_wasm_bg.wasm"
  if [[ ! -f "${path}" ]]; then
    echo "Missing ${path} — run ./scripts/build-wasm.sh first" >&2
    exit 1
  fi
}

if [[ -z "${MAIN_ONLY}" ]]; then
  require_bin vanity-address-darwin-arm64 vanity-address
  require_bin vanity-address-darwin-x64 vanity-address
  require_bin vanity-address-linux-x64 vanity-address
  require_bin vanity-address-win32-x64 vanity-address.exe
fi

require_wasm nodejs
require_wasm bundler

# Stale binaries in npm/*/bin are not overwritten when prepare-npm.sh fails.
check_host_binary_version() {
  local want host
  want="$(sed -n 's/^version = "\(.*\)"/\1/p' "${ROOT}/vanity-address/Cargo.toml" | head -1)"
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) host=vanity-address-darwin-arm64 ;;
    Darwin-x86_64) host=vanity-address-darwin-x64 ;;
    Linux-x86_64) host=vanity-address-linux-x64 ;;
    *) echo "Skipping binary version check on this host"; return ;;
  esac
  local got
  got="$("${ROOT}/npm/${host}/bin/vanity-address" --version | awk '{print $2}')"
  if [[ "${got}" != "${want}" ]]; then
    echo "npm/${host}/bin/vanity-address is ${got}, expected ${want} — run ./scripts/prepare-npm.sh ${want}" >&2
    exit 1
  fi
}
if [[ -z "${MAIN_ONLY}" ]]; then
  check_host_binary_version
fi

# An expired token makes `npm publish` fail with a misleading 404.
ensure_npm_login() {
  if npm whoami >/dev/null 2>&1; then
    echo "npm user: $(npm whoami)"
    return
  fi
  echo "Not logged in to npm (or token expired) — running npm login"
  npm login
  if ! npm whoami >/dev/null 2>&1; then
    echo "npm login failed — cannot publish" >&2
    exit 1
  fi
  echo "npm user: $(npm whoami)"
}
if [[ -z "${DRY}" ]]; then
  ensure_npm_login
fi

if [[ -z "${MAIN_ONLY}" ]]; then
  for dir in "${PLATFORMS[@]}"; do
    echo "==> Publishing ${dir}"
    (cd "${ROOT}/npm/${dir}" && npm publish --access public ${DRY})
  done
fi

echo "==> Publishing vanity-address (main)"
(cd "${ROOT}/npm/vanity-address" && npm publish --access public ${DRY})

echo "Done."
