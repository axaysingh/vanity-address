# Roadmap — vanity-address 0.7.0

> Major community release. **No “optional / later” features** inside this milestone — only paid audits and “become a wallet” stay out of scope by design.
> Version: **0.6.7 → 0.7.0** (single public tag when all three phases are green).

## One-line pitch

> More Bitcoin-family coins, split-key, puzzle mode, OpenCL GPU (EVM incremental EC; Solana experimental), clearer security story, public benchmarks — still MIT, offline, CLI + desktop + web + npm.

## Phase 1 — Chains + parity foundations _(done)_

| Deliverable | Notes |
| ----------- | ----- |
| Bitcoin-fork table | Verified P2PKH `version` + `wif` bytes; expand `BitcoinLikeGrinder` |
| Wire all surfaces | `MENU_CHAINS`, CLI, desktop `list_chains`, web `chain-meta`, npm `listChains` |
| Puzzle mode | Restricted keyspace grind (vanitygen `-Z` / `-l` style) — CLI + core |
| Docs | `docs/BITCOIN_FORKS.md` (sources), USAGE notes |

**Exit:** forks + puzzle tests green; menu/web/desktop list updated.

## Phase 2 — Privacy + trust _(done)_

| Deliverable | Notes |
| ----------- | ----- |
| Split-key | Gen pubkey → grind part → combine; CLI + desktop commands |
| Export safety | Stronger `--save` / download warnings |
| Security pack | Threat model doc; “not formally audited”; not-a-wallet copy |
| Safety UX | Landing + `/generate/` guidance for large funds |

**Exit:** split-key E2E + security/Safety copy on README/landing/web. ✅

## Phase 3 — Speed + proof + ship _(in tree — blockers cleared for tag)_

| Deliverable | Notes |
| ----------- | ----- |
| OpenCL GPU MVP | **EVM + Solana** grind (CPU re-verify); CLI `--gpu` / `gpu devices` / `gpu bench` + desktop `list_gpus` |
| GPU correctness | EVM 3+ nibble prefixes; Apple OpenCL pattern match inlined; field `feadd`/`femul` fixed |
| Benchmarks | `docs/BENCHMARKS.md` + `scripts/bench.sh` (honest CPU vs GPU — Solana GPU may not win) |
| Release sync | **0.7.0** bumped in tree; Formula/sha after tag |
| Publish | Tag → GitHub Release → Homebrew → crates.io → npm |

**Still open before publish:** `make check-ci` green on CI runners; tag + channel publish.

**Exit:** community announce; all channels on 0.7.0.

## Explicitly out of 0.7.0

- Hiring a paid third-party audit firm (we ship honesty + threat model instead)
- Turning the product into a full multi-chain wallet / custody app
- Inflating chain count with dead / unverified forks
- Copying AGPL vanitygen++ code (independent impl + public byte sources only)

## Implementation order

```text
Phase 1 → Phase 2 → Phase 3 → tag v0.7.0 → publish all platforms
```

Do **not** publish crates/npm/Homebrew until Phase 3 exit criteria pass.
