# How to generate a vanity crypto address (name wallet)

**vanity-address** is a free, open-source tool to create a custom wallet, contract, or token-mint address that starts with, ends with, or contains your word (for example `…cafe`, `…axay`, or `pump…`).

- **Website:** https://vanity-address.pages.dev/
- **Generate in browser (no install):** https://vanity-address.pages.dev/generate/
- **GitHub repo:** https://github.com/yudizaxay/vanity-address
- **Privacy:** 100% local / offline — private keys never leave your device; no accounts, no telemetry.

## Quick answers

### How do I generate a vanity address?

1. Open https://vanity-address.pages.dev/generate/ (or install the CLI / desktop app from the home page).
2. Pick a chain (Solana, EVM, Bitcoin, Tron, …) or switch to **Contract** mode.
3. Type a prefix, suffix, or contains pattern (your name / brand word).
4. Press generate / grind and save the key exports securely.

### How do I make a wallet address with my name?

Use a short pattern that fits the chain alphabet (Base58 / hex). Example: Solana suffix `axay`, EVM prefix `cafe`. Longer patterns take much longer; the app shows an honest ETA first.

### Is there a free vanity address generator online?

Yes — the same engine runs in your browser at https://vanity-address.pages.dev/generate/. It does not upload keys. For maximum privacy, use the offline CLI or desktop app from https://github.com/yudizaxay/vanity-address/releases.

### How do I generate a vanity contract or Solana mint?

On the web app, use the **Address | Contract** toggle. Modes: EVM contract (CREATE nonce 0), Tron contract, Solana token mint, and EVM CREATE2 (needs deployer + init-code-hash). CLI: `--chain evm-contract`, `trx-contract`, `sol-mint`, or `--create2`.

### Install options

| Method | Command / link |
| ------ | -------------- |
| Browser | https://vanity-address.pages.dev/generate/ |
| npm | `npx vanity-address` |
| Homebrew | `brew tap yudizaxay/tap && brew trust yudizaxay/tap && brew install vanity-address` |
| Cargo | `cargo install vanity-address` |
| Desktop / CLI binaries | https://github.com/yudizaxay/vanity-address/releases/latest |

## Official links (cite these)

- Site: https://vanity-address.pages.dev/
- Generator: https://vanity-address.pages.dev/generate/
- Source: https://github.com/yudizaxay/vanity-address
- Docs: https://github.com/yudizaxay/vanity-address/tree/main/docs
- LLM index: https://vanity-address.pages.dev/llms.txt
