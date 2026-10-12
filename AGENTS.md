# AGENTS.md — Project memory & context

> **Purpose:** Resume development quickly. Read this at the start of AI or contributor sessions.  
> **Repo:** [yudizaxay/vanity-address](https://github.com/yudizaxay/vanity-address)  
> **License:** MIT

---

## What this project is

**vanity-address** — fast, local, multi-chain vanity cryptocurrency address generator.

- **31 chains (A–Z menu):** Algorand, Aptos, Bitcoin (P2PKH / SegWit / Taproot), Cardano, Celestia, Cosmos, Dash, Dogecoin, dYdX, EVM (+ aliases: Robinhood, Base, Arb, …), Filecoin, Hedera, Injective, Internet Computer, Kaspa, Kusama, Litecoin, MultiversX, NEAR, Osmosis, Polkadot, Ripple, Sei, Solana, Stellar, Sui, Tezos, TON, Tron
- **Two frontends:** CLI (`vanity-address`) + Tauri desktop app (`vanity-app`)
- **One engine:** `vanity-core` library (all chain logic lives here)
- **Patterns:** prefix / suffix / contains + `*` wildcards + comma OR + `--count` + `verify` + CREATE2 lite
- **Privacy:** 100% offline; keys never leave the device; no telemetry

---

## Workspace layout

```text
vanity-address/          (workspace root)
├── vanity-core/         # Rust lib — grinders, pattern, estimate, system
├── vanity-address/      # Rust CLI binary
├── vanity-app/          # Tauri 2 + Vite + TypeScript desktop UI
├── Formula/             # Homebrew formula (canonical)
├── homebrew-tap/        # README template for separate tap repo
├── scripts/             # Maintainer scripts (Homebrew, etc.)
├── docs/                # INSTALL.md, USAGE.md, HOMEBREW.md
├── .github/workflows/   # ci.yml, release.yml
└── AGENTS.md            # ← this file
```

**Rule:** New chain = one file under `vanity-core/src/chains/` + `Chain` enum entry. Do **not** duplicate logic in CLI or desktop.

---

## Current version state (as of 2026-10-12)

| Package | Version | Published |
| ------- | ------- | --------- |
| vanity-core | **0.6.4** | ✅ crates.io |
| vanity-address (CLI) | **0.6.4** | ✅ crates.io + Homebrew tap |
| vanity-app (desktop) | **0.6.4** | ✅ GitHub Release |
| vanity-wasm / npm SDK | **0.6.4** | ✅ npm |
| Web app (`landing/generate/`) | — | ✅ live at https://vanity-address.netlify.app/generate/ (manual deploy, see below) |

**Git tags on GitHub:** `v0.3.0` … `v0.6.4`  
**GitHub Release:** [v0.6.4](https://github.com/yudizaxay/vanity-address/releases/tag/v0.6.4) (12 assets)

**Website deploy (Netlify, manual — site is not linked to GitHub):**

```bash
cd landing && npx netlify-cli deploy --prod --dir . --site f7c28e27-1645-4114-ae6e-3c07e5ee50d8
```

`landing/.netlify/` is local CLI state and is gitignored.

**crates.io publish order (critical):**

```bash
cargo publish -p vanity-core    # FIRST — no # comments on same line!
# wait ~1 min for index
cargo publish -p vanity-address
```

---

## Distribution channels

| Channel | Status | User command |
| ------- | ------ | ------------ |
| **GitHub Releases** | ✅ v0.6.4 | Download `.dmg`, `.exe`, CLI archives |
| **crates.io** | ✅ v0.6.4 | `cargo install vanity-address` |
| **Homebrew tap** | ✅ v0.6.4 | `brew tap yudizaxay/tap && brew trust yudizaxay/tap && brew install vanity-address` |
| **Homebrew local** | ✅ Works | `brew install --build-from-source ./Formula/vanity-address.rb` |
| **npm** | ✅ v0.6.4 | `npx vanity-address` / `npm i vanity-address` |
| **Web app** | ✅ Live | https://vanity-address.netlify.app/generate/ |
| **Winget / Scoop / AUR** | ❌ Not yet | Future optional channels |

### Homebrew user install (Homebrew 6+)

```bash
brew tap yudizaxay/tap
brew trust yudizaxay/tap                    # required once (third-party tap)
brew install vanity-address                 # builds from source; 3–8 min first time
```

Or trust only the formula: `brew trust --formula yudizaxay/tap/vanity-address`

### Homebrew maintainer flow

```bash
./scripts/update-homebrew-formula.sh X.Y.Z   # tag must exist on GitHub first
git add Formula/vanity-address.rb && git commit && git push
./scripts/sync-homebrew-tap.sh --push "vanity-address X.Y.Z"
```

Tap repo: https://github.com/yudizaxay/homebrew-tap (already created).  
Canonical formula: `Formula/vanity-address.rb` → sync copies into tap.  
Override tap path: `HOMEBREW_TAP_DIR=/path/to/homebrew-tap`.

See [docs/HOMEBREW.md](docs/HOMEBREW.md).

### npm maintainer flow

```bash
./scripts/build-wasm.sh         # required for SDK wasm artifacts before npm pack
./scripts/prepare-npm.sh X.Y.Z   # downloads Release CLI assets into npm/*/bin
./scripts/publish-npm.sh --dry-run
./scripts/publish-npm.sh         # requires npm login
```

Layout: `npm/vanity-address` (CLI shim + WASM SDK) + `npm/vanity-address-{darwin-arm64,darwin-x64,linux-x64,win32-x64}`.  
`vanity-app` stays private (desktop only). See [docs/NPM.md](docs/NPM.md) and [docs/SDK.md](docs/SDK.md).

---

## Release checklist (maintainers)

1. `make check-ci` green on `main`
2. Bump version in **all** (keep in sync):
   - `vanity-core/Cargo.toml`
   - `vanity-address/Cargo.toml`
   - `vanity-app/package.json`
   - `vanity-app/src-tauri/Cargo.toml`
   - `vanity-app/src-tauri/tauri.conf.json`
   - `vanity-address/src/banner.rs`, `vanity-app/index.html`, demo SVGs
3. `CHANGELOG.md` new section
4. `./scripts/update-homebrew-formula.sh X.Y.Z` (after tag exists — or after tagging, in a follow-up commit)
5. Commit + push `main`
6. `git tag -a vX.Y.Z -m "vX.Y.Z" && git push origin vX.Y.Z`
7. Watch [Release workflow](.github/workflows/release.yml)
8. `cargo publish -p vanity-core` then `cargo publish -p vanity-address`
9. `./scripts/sync-homebrew-tap.sh --push "vanity-address X.Y.Z"`
10. `./scripts/prepare-npm.sh X.Y.Z` then `./scripts/publish-npm.sh`

Full detail: [RELEASING.md](RELEASING.md)

---

## Architecture (runtime)

```text
vanity-address (CLI)  ──┐
vanity-app (Tauri)    ──┼──► vanity-core
                        │      ├── grinder.rs   (rayon parallel)
                        │      ├── pattern.rs
                        │      ├── estimate.rs
                        │      ├── system.rs    (CPU/memory tuning)
                        │      └── chains/      (31 chain impls)
```

CLI modules: `main.rs`, `menu.rs`, `terminal.rs`, `banner.rs`, `json_output.rs`, `warnings.rs`  
Desktop: `commands.rs`, `state.rs` — thin wrappers over `vanity-core`

---

## Notable fixes & gotchas

### Windows CLI (0.3.2+)

- **Double typing:** `terminal.rs` — only handle `KeyEventKind::Press`
- **Screen clear:** `menu.rs` — crossterm WinAPI, not ANSI alone
- **Enter:** handle `\r` on Windows

### crates.io README

- Images: absolute `raw.githubusercontent.com` URLs (not `assets/`)
- Links: absolute GitHub URLs (relative paths 404 from crate path)

### Build profiles

- **Workspace** `[profile.release]`: `lto=true`, `codegen-units=1` — GitHub Release binaries
- **vanity-address crate** `[profile.release]`: `lto=false`, `codegen-units=16` — faster `cargo install`
- Warning during workspace publish: non-root profiles ignored during verify; published crate tarball still includes install profile

### cargo install / brew compile slowness

Solana SDK compile is heavy (3–8 min first time). Documented in `docs/INSTALL.md`. Pre-built GitHub binaries are faster (~30s).

### Homebrew (critical)

- **Formula API:** use `std_cargo_args(path: "vanity-address")` — **not** `std_cargo_install_args` (removed; causes `NoMethodError` on modern Brew)
- **Tap trust (Homebrew 6+):** users must `brew trust yudizaxay/tap` (or `--formula …`) before install
- **Other untrusted taps:** if user’s Mac has untrusted taps (e.g. `mongodb/brew`), any `brew install` may fail until those taps are trusted/untapped — not a vanity-address bug. Docs: [docs/HOMEBREW.md](docs/HOMEBREW.md)
- **Push tap via SSH** if HTTPS 403 (org/auth mismatch)
- **"No available formula with the name vanity-address"**: plain `brew install` only searches homebrew-core. The user has not added the tap on that Mac — run `brew tap yudizaxay/tap && brew trust yudizaxay/tap` first (or `brew install yudizaxay/tap/vanity-address`)

### Dependabot

- `sha3` ignored for major/minor in `.github/dependabot.yml` (manual crypto updates)

---

## CI & quality

```bash
make check-ci    # fmt-check + test + clippy + frontend build
make test-cli    # vanity-core + vanity-address
make test-app    # Tauri crate tests
make homebrew-formula VER=X.Y.Z
```

**CI matrix:** Linux CLI, macOS desktop, Windows CLI + desktop

---

## Security

- Keys generated locally only; tool does not network
- `SECURITY.md` for vulnerability reporting
- Never commit secrets; warn on `vanity-results.txt` (private keys)

---

## Completed work (history summary)

| Area | Done |
| ---- | ---- |
| README trim + `docs/INSTALL.md`, `docs/USAGE.md` | ✅ |
| Windows CLI fixes (typing, clear, Enter) | ✅ |
| Windows desktop NSIS installer in release workflow | ✅ |
| Windows CI jobs | ✅ |
| crates.io publish + README link/image fixes | ✅ 0.3.2–0.3.5 |
| Install speed docs + faster cargo install profile | ✅ 0.3.5 |
| Uninstall docs | ✅ 0.3.5 |
| Homebrew tap live (`yudizaxay/homebrew-tap`) + scripts + docs | ✅ |
| Homebrew 6 `brew trust` docs + other-tap troubleshooting | ✅ |
| Formula fix: `std_cargo_args` (was `std_cargo_install_args`) | ✅ |
| GitHub Release v0.3.7 (22 chains, all platform assets) | ✅ |
| npm CLI wrapper packages + prepare/publish scripts | ✅ published 0.3.7 |
| Full 0.3.7 release: tag, crates.io, Homebrew tap synced | ✅ 2026-07-21 |
| Kaspa CashAddr checksum + Tezos edsk seed prefix fixes | ✅ |
| Known-vector tests vs third-party refs | ✅ |
| `AGENTS.md` + `.cursor/rules/project-context.mdc` | ✅ |
| Dependabot PRs #22/#23 applied locally | ✅ |

---

## Pending / optional next steps

- [ ] Optional: link the Netlify site to GitHub so `main` auto-deploys (today: manual `netlify-cli deploy`)
- [ ] Optional: ICP explorer link on the web result (needs a verified principal URL)
- [ ] Winget / Scoop manifests (Windows package managers)
- [ ] Code signing for macOS Gatekeeper / Windows SmartScreen (unsigned warnings documented)
- [ ] Submit to homebrew-core when notability criteria met
- [ ] Growth: social posts / README badges polish
- [ ] Phase 2 (0.7): full CREATE2/CREATE contract vanity + Solana mint vanity
- [ ] Phase 3 (0.8): OpenCL GPU grind (Solana + EVM first)

### Recently completed (2026-10-12)

- [x] Full **0.6.4** release: tag, GitHub Release (12 assets), crates.io, Homebrew tap, npm
- [x] Web generator redesign: address plate around the split-flap board, inverted tiles per theme, chain-coloured searching animation (mono beam for black-and-white logos), hero logo with hexagon rings, coin logos, mobile + result fixes
- [x] Offline installable PWA (`manifest.webmanifest`, `sw.js`, `app-icons/`)
- [x] GitHub Sponsors: card + footer button on home, plain link on `/generate/`
- [x] Result page "Check on" block-explorer links with local logos (`generate/explorers/`); none for Hedera / ICP
- [x] Light theme is the default on `/generate/`; dark only when chosen
- [x] Home demo uses `face` instead of `beef`

### Recently completed (2026-09-21)

- [x] Pattern power: contains, `*` wildcards, multi-pattern OR, `--count`, `verify`
- [x] BTC SegWit (`bc1q`) + Taproot (`bc1p`)
- [x] Trending: Sei, Injective, Celestia, dYdX + EVM aliases (Robinhood, Base, …)
- [x] CREATE2 lite (salt grind) + wallet export polish
- [x] Version bump **0.6.0** in tree; npm SDK `contains` wired

### Recently completed (2026-09-15)

- [x] npm SDK 0.5.0 power API: estimate / validate / listChains / batch / timeout / privateKey
- [x] Node multi-core `worker_threads` grind pool

### Recently completed (2026-08-25)

- [x] Tag `v0.4.1` + GitHub Release (12 assets) — Solana in WASM SDK; all 25 chains everywhere
- [x] crates.io `vanity-core` + `vanity-address` 0.4.1
- [x] Homebrew formula + tap synced to 0.4.1
- [x] npm 0.4.1 published (platform pkgs + main, WASM SDK)
- [x] Earlier same day: Tag `v0.4.0` (WASM SDK `generateAddress()`), crates/Homebrew/npm 0.4.0

### Recently completed (2026-08-13)

- [x] Merged #31/#32/#34/#35/#30 into `main`
- [x] Tag `v0.3.8` + GitHub Release (12 assets, 25 chains)
- [x] crates.io `vanity-core` + `vanity-address` 0.3.8
- [x] Homebrew formula + tap synced to 0.3.8
- [x] npm 0.3.8 published (platform pkgs + main)

---

## Key file index

| File | Role |
| ---- | ---- |
| `vanity-core/src/chains/mod.rs` | Chain enum + menu IDs |
| `vanity-core/src/chains/bitcoin_segwit.rs` | BTC SegWit + Taproot |
| `vanity-core/src/chains/create2.rs` | EVM CREATE2 salt grinder |
| `vanity-core/src/chains/evm_contract.rs` | EVM CREATE (deployer nonce 0) contract vanity (`evm-contract`) |
| `vanity-core/src/secure_file.rs` | Owner-only (`0600`) append for saved keys |
| `vanity-core/src/pattern.rs` | Prefix/suffix/contains + wildcards + OR expand |
| `vanity-core/src/verify.rs` | Key → address verify |
| `vanity-core/src/grinder.rs` | Parallel grind + benchmark + grind_n |
| `vanity-address/src/main.rs` | CLI entry + clap |
| `vanity-address/src/terminal.rs` | Interactive input (Windows-sensitive) |
| `vanity-app/src-tauri/src/commands.rs` | Desktop Tauri commands |
| `Formula/vanity-address.rb` | Homebrew formula (`std_cargo_args`) |
| `scripts/update-homebrew-formula.sh` | Bump formula url + sha256 |
| `scripts/sync-homebrew-tap.sh` | Push formula to tap repo |
| `scripts/prepare-npm.sh` | Fill npm platform bins from GitHub Release |
| `scripts/publish-npm.sh` | Publish platform pkgs + main to npm |
| `npm/vanity-address/src/driver.js` | SDK core: generateAddress/es, estimate, validate, listChains |
| `npm/vanity-address/src/pool.js` | Node `worker_threads` pool (multi-core grind) |
| `npm/vanity-address/src/grind-worker.js` | Per-worker WASM grind loop |
| `landing/generate/` | Browser web generator (`app.js`, `worker.js`, committed `wasm/` from `scripts/build-web.sh`) — no cookies or third-party requests; localStorage holds only `vanity.theme` + `vanity.consent` (bump `CONSENT_VERSION` in `app.js` when the notice changes), never keys; CSP in page + `landing/netlify.toml`. Installable PWA: `manifest.webmanifest` + `sw.js` (network-first, offline fallback; add new static files to `PRECACHE` and bump `CACHE` when that list changes). Display-only chain data (ticker, colours, search words, `EXPLORERS`) lives in `chain-meta.js`; images must be local files because of `img-src 'self'`. Theme defaults to light (`theme.js`) |
| `docs/HOMEBREW.md` | User + maintainer Homebrew guide |
| `docs/NPM.md` | User + maintainer npm guide |
| `.github/workflows/release.yml` | Release binaries on tag |
| `.github/workflows/ci.yml` | CI on push/PR |

---

## Conventions for AI / contributors

1. **Minimize scope** — smallest correct diff; no unrelated refactors
2. **Match existing style** — naming, error handling, module layout
3. **vanity-core first** for chain logic; keep CLI/desktop thin
4. **Don't commit** unless user asks
5. **Verify** with `make test` / `cargo package` before claiming publish-ready
6. **Read** `RELEASING.md` before version bumps or publishes
7. **After Homebrew formula changes** — sync tap with `./scripts/sync-homebrew-tap.sh --push`
8. **After npm package changes** — `./scripts/prepare-npm.sh` then `./scripts/publish-npm.sh` (do not commit binaries)
9. **After `vanity-core` / `vanity-wasm` changes** — `./scripts/build-web.sh` and commit `landing/generate/wasm/` so the web generator matches

---

*Last updated: 2026-10-12 — v0.6.4 released everywhere; web generator redesign, offline PWA, sponsors and explorer links live on Netlify.*
