# Bitcoin-family P2PKH coins

Phase 1 of the [0.7.0 roadmap](ROADMAP-0.7.md). These coins share secp256k1 + Base58Check P2PKH; only the **address version** and **WIF** bytes differ.

**Rule:** we only ship forks whose version byte yields a **stable first address character** (so pattern “lead” injection stays correct). Coins like Namecoin / Feathercoin (first char can vary) are omitted until we support multi-lead layouts.

Sources are public chainparams / SLIP-style docs — **not** AGPL vanitygen++ source.

## Core (already in 0.6.x)

| Id | Coin | Version | WIF | Lead |
| -- | ---- | ------- | --- | ---- |
| `btc` | Bitcoin | `0x00` | `0x80` | `1` |
| `ltc` | Litecoin | `0x30` | `0xb0` | `L` |
| `doge` | Dogecoin | `0x1e` | `0x9e` | `D` |
| `dash` | Dash | `0x4c` | `0xcc` | `X` |

## Extended (Phase 1)

| Id | Coin | Version | WIF | Lead |
| -- | ---- | ------- | --- | ---- |
| `btg` | Bitcoin Gold | `0x26` | `0x80` | `G` |
| `rvn` | Ravencoin | `0x3c` | `0x80` | `R` |
| `mona` | Monacoin | `0x32` | `0xb0` | `M` |
| `vtc` | Vertcoin | `0x47` | `0x80` | `V` |
| `dgb` | DigiByte | `0x1e` | `0x80` | `D` |
| `ppc` | Peercoin | `0x37` | `0xb7` | `P` |
| `qtum` | Qtum | `0x3a` | `0x80` | `Q` |
| `via` | Viacoin | `0x47` | `0xc7` | `V` |
| `xpm` | Primecoin | `0x17` | `0x97` | `A` |
| `emc2` | Einsteinium | `0x21` | `0xa1` | `E` |
| `sys` | Syscoin | `0x3f` | `0xbf` | `S` |
| `nvc` | Novacoin | `0x08` | `0x88` | `4` |
| `cdn` | Canada eCoin | `0x1c` | `0x9c` | `C` |

## CLI examples

```bash
vanity-address --chain rvn --prefix RLove
vanity-address --chain btg --suffix Gold
```

## Puzzle mode (Bitcoin)

Restricted private-key search (high bits fixed, low `N` bits free):

```bash
# ~2^6 keyspace, zero start, match address prefix
vanity-address --puzzle 6 --prefix 1Love

# custom start template (32-byte hex)
vanity-address --puzzle 20 --puzzle-start 0000000000000000000000000000000000000000000000000000000000000000 --prefix 1
```

`--puzzle` currently supports `--chain btc` only (defaulted).
