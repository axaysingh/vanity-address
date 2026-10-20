# Security model

Honest threat model for **vanity-address**. This is a **local key generator**, not a wallet, custodian, or audited vault.

## What this software is

- Offline vanity address grinder (CLI, desktop, browser WASM, npm SDK)
- Produces key material on the device you run it on
- Open source (MIT) — you can read every line of `vanity-core`

## What this software is not

| Claim people sometimes assume | Reality |
| ----------------------------- | ------- |
| “Formally audited” | **Not** hired for a paid third-party audit. Review the code yourself or hire your own auditor. |
| “A wallet” | No balances, no signing UX, no seed backup flow, no hardware-wallet bridge. Import keys into a wallet **you** trust after verifying the address. |
| “Safe for large funds by default” | Host malware, clipboard stealers, shared screens, and careless `--save` / downloads can lose everything. Prefer air-gapped CLI for meaningful balances. |
| “Recovers lost keys” | Impossible. We never had a copy. |

## Trust boundaries

```text
┌─────────────────────────────────────────────────────────┐
│  Your machine (OS, malware, other users, backups)       │
│    ┌───────────────────────────────────────────────┐    │
│    │  vanity-address process                       │    │
│    │  · RNG → grind → finalize                     │    │
│    │  · optional plaintext --save / download       │    │
│    └───────────────────────────────────────────────┘    │
│  No network during grind (by design)                    │
└─────────────────────────────────────────────────────────┘
         ▲
         │  You import key / verify address
         ▼
   External wallet / chain (out of scope)
```

**In scope for us:** correct address derivation for supported chains; no telemetry; owner-only file mode on Unix when saving.

**Out of scope:** HSM, encrypted vaults, anti-malware, side-channel resistance, social engineering.

## Split-key mode

Alice keeps `sk_a`, shares only `pk_a`. Bob grinds `sk_b` so `address(pk_a ⊕ pk_b)` matches. Alice alone can form `sk = sk_a + sk_b (mod n)`.

- Bob never learns the final spending key.
- Alice must still protect `sk_a` and the combined key after `split-key combine`.
- Independent MIT implementation of the well-known split-key idea — not a copy of AGPL vanitygen++.

## Safer habits for large funds

1. Prefer **CLI or desktop** on a clean / air-gapped machine over a daily-driver browser.
2. Use **short patterns** you can actually finish; do not grind for years on a hot laptop.
3. Treat `--save` / downloads as **plaintext secrets** — encrypt, keep offline, shred extras.
4. Always **`verify`** (or import into a known-good wallet and check the address) before sending value.
5. For untrusted grinders, use **split-key** so the helper never holds the final private key.

## Related

- Vulnerability reporting: [SECURITY.md](../SECURITY.md)
- Bitcoin-family bytes / puzzle: [BITCOIN_FORKS.md](BITCOIN_FORKS.md)
- Usage: [USAGE.md](USAGE.md)
