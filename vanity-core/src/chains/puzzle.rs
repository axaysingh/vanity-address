//! Bitcoin puzzle-style grind: search a restricted secp256k1 keyspace.
//!
//! Compatible with the common “puzzle N” framing (≈`2^N` keys): the high
//! `256 - N` bits of the private key are fixed (usually zero) and only the
//! low `N` bits are random. Independent of AGPL vanitygen++ — public puzzle
//! definition only.

use crate::chain::{ChainGrinder, GrindAttempt, KeyExport, KeypairResult};
use crate::pattern::Pattern;
use rand::rngs::OsRng;
use rand::RngCore;
use secp256k1::SecretKey;

use super::bitcoin_like::BitcoinLikeGrinder;
use super::util::p2pkh_address;

#[derive(Clone)]
pub struct PuzzleGrinder {
    /// Number of free low bits (puzzle number ≈ free bits).
    pub free_bits: u32,
    /// Fixed high-bit template (usually all zeros). Low `free_bits` are ignored.
    pub start: [u8; 32],
    inner: BitcoinLikeGrinder,
}

impl PuzzleGrinder {
    /// Bitcoin P2PKH puzzle with zero start and `free_bits` free low bits.
    pub fn bitcoin(free_bits: u32) -> Result<Self, String> {
        Self::new(free_bits, [0u8; 32], BitcoinLikeGrinder::bitcoin())
    }

    pub fn new(free_bits: u32, start: [u8; 32], inner: BitcoinLikeGrinder) -> Result<Self, String> {
        if free_bits == 0 || free_bits > 256 {
            return Err("puzzle free bits must be in 1..=256".into());
        }
        Ok(Self {
            free_bits,
            start,
            inner,
        })
    }

    /// Parse a 32-byte hex start (with optional `0x`).
    pub fn parse_start(hex: &str) -> Result<[u8; 32], String> {
        let s = hex.trim().trim_start_matches("0x").trim_start_matches("0X");
        if s.len() != 64 {
            return Err("puzzle start must be 32-byte hex (64 hex chars)".into());
        }
        let mut out = [0u8; 32];
        for i in 0..32 {
            out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
                .map_err(|_| "puzzle start is not valid hex".to_string())?;
        }
        Ok(out)
    }

    fn sample_secret(&self) -> SecretKey {
        loop {
            let bytes = sample_key_bytes(&self.start, self.free_bits);
            if let Ok(sk) = SecretKey::from_slice(&bytes) {
                return sk;
            }
        }
    }
}

/// Keep high `256 - free_bits` from `start`; replace low `free_bits` with random.
fn sample_key_bytes(start: &[u8; 32], free_bits: u32) -> [u8; 32] {
    let mut out = *start;
    if free_bits >= 256 {
        OsRng.fill_bytes(&mut out);
        return out;
    }
    let free = free_bits as usize;
    let mut rnd = [0u8; 32];
    OsRng.fill_bytes(&mut rnd);

    // Clear low `free` bits of `out`, then OR random low bits.
    let full_bytes = free / 8;
    let rem = free % 8;
    for i in 0..full_bytes {
        out[31 - i] = rnd[31 - i];
    }
    if rem > 0 {
        let idx = 31 - full_bytes;
        let mask = (1u8 << rem) - 1;
        out[idx] = (out[idx] & !mask) | (rnd[idx] & mask);
    }
    out
}

impl ChainGrinder for PuzzleGrinder {
    fn id(&self) -> &'static str {
        "btc-puzzle"
    }

    fn display_name(&self) -> &'static str {
        "Bitcoin puzzle (restricted keyspace)"
    }

    fn grind_attempt(&self) -> (String, GrindAttempt) {
        let secret = self.sample_secret();
        let address = p2pkh_address(&secret, self.inner.version_byte);
        (address, GrindAttempt::Secret32(secret.secret_bytes()))
    }

    fn finalize(&self, attempt: GrindAttempt) -> KeypairResult {
        let mut result = self.inner.finalize(attempt);
        result.exports.insert(
            0,
            KeyExport {
                label: "Puzzle".into(),
                value: format!(
                    "{} free bits · start {}",
                    self.free_bits,
                    hex::encode(self.start)
                ),
                hint: Some(
                    "Restricted keyspace grind (Bitcoin puzzle style). Keep the WIF safe.".into(),
                ),
            },
        );
        result
    }

    fn build_pattern(
        &self,
        prefix: Option<&str>,
        suffix: Option<&str>,
        contains: Option<&str>,
        exact: bool,
    ) -> Result<Pattern, String> {
        self.inner.build_pattern(prefix, suffix, contains, exact)
    }

    fn expected_attempts(&self, pattern: &Pattern) -> f64 {
        // Address pattern odds × restricted keyspace does not multiply cleanly for ETA;
        // report address-pattern attempts (same as normal BTC) — keyspace is a search mode.
        self.inner.expected_attempts(pattern)
    }

    fn matches(&self, address: &str, pattern: &Pattern) -> bool {
        self.inner.matches(address, pattern)
    }

    fn supports_exact_case(&self) -> bool {
        true
    }

    fn pattern_hint(&self) -> &'static str {
        "Bitcoin P2PKH puzzle mode: private key low bits are free; high bits fixed (usually zero)."
    }
}

#[cfg(test)]
mod tests {
    use super::{sample_key_bytes, PuzzleGrinder};
    use crate::chain::ChainGrinder;

    #[test]
    fn free_bits_only_touch_low_end() {
        let start = [0u8; 32];
        let b = sample_key_bytes(&start, 8);
        assert_eq!(&b[..31], &[0u8; 31]);
    }

    #[test]
    fn puzzle6_grinds_btc_address() {
        let g = PuzzleGrinder::bitcoin(6).unwrap();
        let (addr, attempt) = g.grind_attempt();
        assert!(addr.starts_with('1'));
        let fin = g.finalize(attempt);
        assert_eq!(fin.address, addr);
        assert!(fin.exports.iter().any(|e| e.label == "Puzzle"));
    }

    #[test]
    fn rejects_zero_bits() {
        assert!(PuzzleGrinder::bitcoin(0).is_err());
    }
}
