use crate::chain::{ChainGrinder, GrindAttempt, KeyExport, KeypairResult};
use crate::pattern::Pattern;
use secp256k1::SecretKey;

use super::base58_odds::Base58Layout;
use super::util::{
    build_base58_pattern, grind_secp256k1, matches_pattern, p2pkh_address, secret_from_attempt,
    BASE58_ALPHABET,
};

/// Static P2PKH Bitcoin-family coin (version + WIF bytes from public chainparams).
#[derive(Clone, Copy, Debug)]
pub struct BitcoinForkSpec {
    pub id: &'static str,
    pub display_name: &'static str,
    pub version_byte: u8,
    pub wif_byte: u8,
    /// First character every mainnet P2PKH address gets from `version_byte`.
    pub lead: &'static str,
    pub wallet_hint: &'static str,
}

#[derive(Clone)]
pub struct BitcoinLikeGrinder {
    pub id: &'static str,
    pub display_name: &'static str,
    pub version_byte: u8,
    pub wif_byte: u8,
    pub lead: &'static str,
    pub wallet_hint: &'static str,
    pub pattern_hint: &'static str,
}

impl BitcoinLikeGrinder {
    fn from_spec(spec: BitcoinForkSpec) -> Self {
        Self {
            id: spec.id,
            display_name: spec.display_name,
            version_byte: spec.version_byte,
            wif_byte: spec.wif_byte,
            lead: spec.lead,
            wallet_hint: spec.wallet_hint,
            pattern_hint: hint_for_lead(spec.lead),
        }
    }

    pub fn bitcoin() -> Self {
        Self::from_spec(BITCOIN)
    }

    pub fn litecoin() -> Self {
        Self::from_spec(LITECOIN)
    }

    pub fn dogecoin() -> Self {
        Self::from_spec(DOGECOIN)
    }

    pub fn dash() -> Self {
        Self::from_spec(DASH)
    }

    /// Look up any registered Bitcoin-family P2PKH coin (core four + extended forks).
    pub fn by_id(id: &str) -> Option<Self> {
        let id = id.trim().to_ascii_lowercase();
        ALL_FORKS
            .iter()
            .find(|s| s.id == id)
            .copied()
            .map(Self::from_spec)
    }

    /// Extended forks only (everything beyond btc/ltc/doge/dash) — for menu append.
    pub fn extended_forks() -> impl Iterator<Item = Self> {
        EXTENDED_FORKS.iter().copied().map(Self::from_spec)
    }

    /// Version byte + 20-byte hash160 + 4-byte checksum.
    fn layout(&self) -> Base58Layout<'_> {
        Base58Layout {
            version: std::slice::from_ref(&self.version_byte),
            random_len: 24,
            alphabet: BASE58_ALPHABET,
        }
    }

    fn derive(&self, secret: &SecretKey) -> String {
        p2pkh_address(secret, self.version_byte)
    }
}

fn hint_for_lead(lead: &str) -> &'static str {
    // Leads are single ASCII chars from the table; keep hints static for ChainGrinder.
    match lead {
        "1" => "Base58 (no 0, O, I, l). Addresses always start with 1 (added for you).",
        "L" => "Base58 (no 0, O, I, l). Addresses always start with L (added for you).",
        "D" => "Base58 (no 0, O, I, l). Addresses always start with D (added for you).",
        "X" => "Base58 (no 0, O, I, l). Addresses always start with X (added for you).",
        "G" => "Base58 (no 0, O, I, l). Addresses always start with G (added for you).",
        "R" => "Base58 (no 0, O, I, l). Addresses always start with R (added for you).",
        "M" => "Base58 (no 0, O, I, l). Addresses always start with M (added for you).",
        "V" => "Base58 (no 0, O, I, l). Addresses always start with V (added for you).",
        "N" => "Base58 (no 0, O, I, l). Addresses always start with N (added for you).",
        "P" => "Base58 (no 0, O, I, l). Addresses always start with P (added for you).",
        "Q" => "Base58 (no 0, O, I, l). Addresses always start with Q (added for you).",
        "A" => "Base58 (no 0, O, I, l). Addresses always start with A (added for you).",
        "E" => "Base58 (no 0, O, I, l). Addresses always start with E (added for you).",
        "S" => "Base58 (no 0, O, I, l). Addresses always start with S (added for you).",
        "4" => "Base58 (no 0, O, I, l). Addresses always start with 4 (added for you).",
        "C" => "Base58 (no 0, O, I, l). Addresses always start with C (added for you).",
        _ => "Base58 (no 0, O, I, l). Leading version character is added for you.",
    }
}

// --- Core four (already shipped) -------------------------------------------------

const BITCOIN: BitcoinForkSpec = BitcoinForkSpec {
    id: "btc",
    display_name: "Bitcoin (P2PKH)",
    version_byte: 0x00,
    wif_byte: 0x80,
    lead: "1",
    wallet_hint: "Electrum / Sparrow / hardware wallet WIF import",
};

const LITECOIN: BitcoinForkSpec = BitcoinForkSpec {
    id: "ltc",
    display_name: "Litecoin (P2PKH)",
    version_byte: 0x30,
    wif_byte: 0xb0,
    lead: "L",
    wallet_hint: "Litecoin Core / compatible wallets",
};

const DOGECOIN: BitcoinForkSpec = BitcoinForkSpec {
    id: "doge",
    display_name: "Dogecoin",
    version_byte: 0x1e,
    wif_byte: 0x9e,
    lead: "D",
    wallet_hint: "Dogecoin Core / compatible wallets",
};

const DASH: BitcoinForkSpec = BitcoinForkSpec {
    id: "dash",
    display_name: "Dash (P2PKH)",
    version_byte: 0x4c,
    wif_byte: 0xcc,
    lead: "X",
    wallet_hint: "Dash Core / compatible wallets",
};

// --- Extended forks (Phase 1) — bytes from public chainparams / SLIP docs --------
// Do not copy AGPL vanitygen++ source; verify against coin docs before expanding.

const BITCOIN_GOLD: BitcoinForkSpec = BitcoinForkSpec {
    id: "btg",
    display_name: "Bitcoin Gold (P2PKH)",
    version_byte: 0x26,
    wif_byte: 0x80,
    lead: "G",
    wallet_hint: "Bitcoin Gold compatible WIF import",
};

const RAVENCOIN: BitcoinForkSpec = BitcoinForkSpec {
    id: "rvn",
    display_name: "Ravencoin (P2PKH)",
    version_byte: 0x3c,
    wif_byte: 0x80,
    lead: "R",
    wallet_hint: "Ravencoin Core / compatible wallets",
};

const MONACOIN: BitcoinForkSpec = BitcoinForkSpec {
    id: "mona",
    display_name: "Monacoin (P2PKH)",
    version_byte: 0x32,
    wif_byte: 0xb0,
    lead: "M",
    wallet_hint: "Monacoin compatible WIF import",
};

const VERTCOIN: BitcoinForkSpec = BitcoinForkSpec {
    id: "vtc",
    display_name: "Vertcoin (P2PKH)",
    version_byte: 0x47,
    wif_byte: 0x80,
    lead: "V",
    wallet_hint: "Vertcoin Core / compatible wallets",
};

const DIGIBYTE: BitcoinForkSpec = BitcoinForkSpec {
    id: "dgb",
    display_name: "DigiByte (P2PKH)",
    version_byte: 0x1e,
    wif_byte: 0x80,
    lead: "D",
    wallet_hint: "DigiByte Core / compatible wallets",
};

const PEERCOIN: BitcoinForkSpec = BitcoinForkSpec {
    id: "ppc",
    display_name: "Peercoin (P2PKH)",
    version_byte: 0x37,
    wif_byte: 0xb7,
    lead: "P",
    wallet_hint: "Peercoin compatible WIF import",
};

const QTUM: BitcoinForkSpec = BitcoinForkSpec {
    id: "qtum",
    display_name: "Qtum (P2PKH)",
    version_byte: 0x3a,
    wif_byte: 0x80,
    lead: "Q",
    wallet_hint: "Qtum Core / compatible wallets",
};

const VIACOIN: BitcoinForkSpec = BitcoinForkSpec {
    id: "via",
    display_name: "Viacoin (P2PKH)",
    version_byte: 0x47,
    wif_byte: 0xc7,
    lead: "V",
    wallet_hint: "Viacoin compatible WIF import",
};

const PRIMECOIN: BitcoinForkSpec = BitcoinForkSpec {
    id: "xpm",
    display_name: "Primecoin (P2PKH)",
    version_byte: 0x17,
    wif_byte: 0x97,
    lead: "A",
    wallet_hint: "Primecoin compatible WIF import",
};

const EINSTEINIUM: BitcoinForkSpec = BitcoinForkSpec {
    id: "emc2",
    display_name: "Einsteinium (P2PKH)",
    version_byte: 0x21,
    wif_byte: 0xa1,
    lead: "E",
    wallet_hint: "Einsteinium compatible WIF import",
};

const SYSCOIN: BitcoinForkSpec = BitcoinForkSpec {
    id: "sys",
    display_name: "Syscoin (P2PKH)",
    version_byte: 0x3f,
    wif_byte: 0xbf,
    lead: "S",
    wallet_hint: "Syscoin compatible WIF import",
};

const NOVACOIN: BitcoinForkSpec = BitcoinForkSpec {
    id: "nvc",
    display_name: "Novacoin (P2PKH)",
    version_byte: 0x08,
    wif_byte: 0x88,
    lead: "4",
    wallet_hint: "Novacoin compatible WIF import",
};

const CANADA_ECOIN: BitcoinForkSpec = BitcoinForkSpec {
    id: "cdn",
    display_name: "Canada eCoin (P2PKH)",
    version_byte: 0x1c,
    wif_byte: 0x9c,
    lead: "C",
    wallet_hint: "Canada eCoin compatible WIF import",
};

/// Only coins whose P2PKH version byte yields a **stable** first address character.
/// (Skipped e.g. Namecoin / Feathercoin — first char can be M|N or 6|7.)
const EXTENDED_FORKS: &[BitcoinForkSpec] = &[
    BITCOIN_GOLD,
    RAVENCOIN,
    MONACOIN,
    VERTCOIN,
    DIGIBYTE,
    PEERCOIN,
    QTUM,
    VIACOIN,
    PRIMECOIN,
    EINSTEINIUM,
    SYSCOIN,
    NOVACOIN,
    CANADA_ECOIN,
];

const ALL_FORKS: &[BitcoinForkSpec] = &[
    BITCOIN,
    LITECOIN,
    DOGECOIN,
    DASH,
    BITCOIN_GOLD,
    RAVENCOIN,
    MONACOIN,
    VERTCOIN,
    DIGIBYTE,
    PEERCOIN,
    QTUM,
    VIACOIN,
    PRIMECOIN,
    EINSTEINIUM,
    SYSCOIN,
    NOVACOIN,
    CANADA_ECOIN,
];

impl ChainGrinder for BitcoinLikeGrinder {
    fn id(&self) -> &'static str {
        self.id
    }

    fn display_name(&self) -> &'static str {
        self.display_name
    }

    fn grind_attempt(&self) -> (String, GrindAttempt) {
        grind_secp256k1(|secret| self.derive(secret))
    }

    fn finalize(&self, attempt: GrindAttempt) -> KeypairResult {
        let secret_bytes = secret_from_attempt(attempt);
        let secret_key = SecretKey::from_slice(&secret_bytes).expect("valid secp256k1 secret");
        let address = self.derive(&secret_key);

        KeypairResult {
            address,
            exports: vec![
                KeyExport {
                    label: "Private Key (hex)".into(),
                    value: hex::encode(secret_bytes),
                    hint: Some(self.wallet_hint.into()),
                },
                KeyExport {
                    label: "Private Key (WIF)".into(),
                    value: wif_encode(secret_bytes, self.wif_byte),
                    hint: Some("Standard Bitcoin-family WIF format".into()),
                },
            ],
        }
    }

    fn build_pattern(
        &self,
        prefix: Option<&str>,
        suffix: Option<&str>,
        contains: Option<&str>,
        exact: bool,
    ) -> Result<Pattern, String> {
        let mut pattern =
            build_base58_pattern(prefix, suffix, contains, exact, BASE58_ALPHABET, 34)?;
        self.layout()
            .apply_lead(self.display_name, &mut pattern, self.lead)?;
        Ok(pattern)
    }

    fn expected_attempts(&self, pattern: &Pattern) -> f64 {
        self.layout().expected_attempts(pattern)
    }

    fn matches(&self, address: &str, pattern: &Pattern) -> bool {
        matches_pattern(address, pattern, false)
    }

    fn supports_exact_case(&self) -> bool {
        true
    }

    fn pattern_hint(&self) -> &'static str {
        self.pattern_hint
    }
}

fn wif_encode(secret: [u8; 32], version: u8) -> String {
    let mut payload = vec![version];
    payload.extend_from_slice(&secret);
    payload.push(0x01); // compressed pubkey flag
    super::util::base58_check_encode_raw(&payload)
}

#[cfg(test)]
mod tests {
    use super::{BitcoinLikeGrinder, ALL_FORKS, EXTENDED_FORKS};
    use crate::chain::ChainGrinder;
    use crate::chains::util::{p2pkh_address, random_secp256k1_secret};

    #[test]
    fn core_leads() {
        let secret = random_secp256k1_secret();
        assert!(
            p2pkh_address(&secret, BitcoinLikeGrinder::bitcoin().version_byte).starts_with('1')
        );
        assert!(
            p2pkh_address(&secret, BitcoinLikeGrinder::litecoin().version_byte).starts_with('L')
        );
        assert!(
            p2pkh_address(&secret, BitcoinLikeGrinder::dogecoin().version_byte).starts_with('D')
        );
        assert!(p2pkh_address(&secret, BitcoinLikeGrinder::dash().version_byte).starts_with('X'));
    }

    #[test]
    fn every_fork_lead_is_stable_across_keys() {
        for spec in ALL_FORKS {
            for _ in 0..64 {
                let secret = random_secp256k1_secret();
                let addr = p2pkh_address(&secret, spec.version_byte);
                assert!(
                    addr.starts_with(spec.lead),
                    "{}: expected lead {:?}, got {}",
                    spec.id,
                    spec.lead,
                    addr
                );
            }
        }
    }

    #[test]
    fn by_id_resolves_extended_forks() {
        for spec in EXTENDED_FORKS {
            let g = BitcoinLikeGrinder::by_id(spec.id).expect(spec.id);
            assert_eq!(g.id(), spec.id);
            let (addr, attempt) = g.grind_attempt();
            assert!(addr.starts_with(spec.lead), "{} → {addr}", spec.id);
            let fin = g.finalize(attempt);
            assert_eq!(fin.address, addr);
            assert!(fin.exports.iter().any(|e| e.label.contains("WIF")));
        }
    }

    #[test]
    fn wif_byte_can_differ_from_version_plus_0x80() {
        let rvn = BitcoinLikeGrinder::by_id("rvn").unwrap();
        assert_eq!(rvn.wif_byte, 0x80);
        assert_ne!(rvn.wif_byte, rvn.version_byte.wrapping_add(0x80));
        let doge = BitcoinLikeGrinder::dogecoin();
        assert_eq!(doge.wif_byte, doge.version_byte.wrapping_add(0x80));
    }
}
