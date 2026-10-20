//! Split-key vanity (secp256k1 / Bitcoin-family P2PKH).
//!
//! Alice keeps `sk_a` secret and shares only `pk_a`. Bob searches `sk_b` such that
//! `address(pk_a + sk_b·G)` matches the pattern. Alice combines
//! `sk = sk_a + sk_b (mod n)` to spend. Independent MIT implementation of the
//! well-known [split-key](https://en.bitcoin.it/wiki/Split-key_vanity_address) idea.

use crate::chain::{ChainGrinder, GrindAttempt, KeyExport, KeypairResult};
use crate::pattern::Pattern;
use rand::rngs::OsRng;
use secp256k1::{PublicKey, Scalar, Secp256k1, SecretKey};

use super::bitcoin_like::BitcoinLikeGrinder;
use super::util::{grind_secp256k1, p2pkh_address, p2pkh_address_from_pubkey, secret_from_attempt};

/// Alice-side material from [`generate_split_keypair`].
#[derive(Clone)]
pub struct SplitKeypair {
    pub address: String,
    pub private_key_hex: String,
    pub private_key_wif: String,
    /// Uncompressed SEC1 pubkey hex (`04` ‖ X ‖ Y) — share this, never the private key.
    pub public_key_hex: String,
}

/// Generate Alice's half: keep the private key; send `public_key_hex` to the grinder.
pub fn generate_split_keypair(coin: &BitcoinLikeGrinder) -> SplitKeypair {
    let secp = Secp256k1::new();
    let secret = SecretKey::new(&mut OsRng);
    let public = secret.public_key(&secp);
    let address = p2pkh_address(&secret, coin.version_byte);
    SplitKeypair {
        address,
        private_key_hex: hex::encode(secret.secret_bytes()),
        private_key_wif: wif_encode(secret.secret_bytes(), coin.wif_byte),
        public_key_hex: hex::encode(public.serialize_uncompressed()),
    }
}

/// Parse compressed (33-byte) or uncompressed (65-byte) SEC1 public key hex.
pub fn parse_public_key(hex: &str) -> Result<PublicKey, String> {
    let s = hex.trim().trim_start_matches("0x").trim_start_matches("0X");
    let bytes = hex::decode(s).map_err(|_| "split public key is not valid hex".to_string())?;
    PublicKey::from_slice(&bytes).map_err(|e| format!("invalid secp256k1 public key: {e}"))
}

/// Parse a 32-byte secret from hex or Bitcoin-family WIF (compressed/uncompressed).
pub fn parse_secret_key(input: &str) -> Result<SecretKey, String> {
    let s = input.trim();
    if let Ok(bytes) = hex::decode(s.trim_start_matches("0x").trim_start_matches("0X")) {
        if bytes.len() == 32 {
            return SecretKey::from_slice(&bytes)
                .map_err(|e| format!("invalid private key hex: {e}"));
        }
    }
    // WIF: Base58Check → version || 32-byte secret [|| 0x01] || 4-byte checksum
    let decoded = bs58::decode(s)
        .into_vec()
        .map_err(|_| "private key must be 32-byte hex or WIF".to_string())?;
    if decoded.len() < 37 {
        return Err("WIF payload too short".into());
    }
    let (payload, checksum) = decoded.split_at(decoded.len() - 4);
    let expect = {
        use sha2::{Digest, Sha256};
        let mid = Sha256::digest(payload);
        let full = Sha256::digest(mid);
        [full[0], full[1], full[2], full[3]]
    };
    if checksum != expect {
        return Err("WIF checksum mismatch (typo or wrong network?)".into());
    }
    // drop version (1); optional compressed flag 0x01
    let body = &payload[1..];
    let secret_bytes = if body.len() == 33 && body[32] == 0x01 {
        &body[..32]
    } else if body.len() == 32 {
        body
    } else {
        return Err("unrecognized WIF payload length".into());
    };
    SecretKey::from_slice(secret_bytes).map_err(|e| format!("invalid WIF secret: {e}"))
}

/// `sk_final = sk_alice + sk_part (mod n)` and the matching P2PKH address.
pub fn combine_split_keys(
    alice_secret: &SecretKey,
    part_secret: &SecretKey,
    coin: &BitcoinLikeGrinder,
) -> Result<SplitKeypair, String> {
    let tweak = Scalar::from_be_bytes(part_secret.secret_bytes())
        .map_err(|_| "part private key is out of range for secp256k1".to_string())?;
    let combined = alice_secret
        .add_tweak(&tweak)
        .map_err(|e| format!("failed to combine private keys: {e}"))?;
    let secp = Secp256k1::new();
    let public = combined.public_key(&secp);
    Ok(SplitKeypair {
        address: p2pkh_address(&combined, coin.version_byte),
        private_key_hex: hex::encode(combined.secret_bytes()),
        private_key_wif: wif_encode(combined.secret_bytes(), coin.wif_byte),
        public_key_hex: hex::encode(public.serialize_uncompressed()),
    })
}

fn wif_encode(secret: [u8; 32], version: u8) -> String {
    let mut payload = vec![version];
    payload.extend_from_slice(&secret);
    payload.push(0x01);
    super::util::base58_check_encode_raw(&payload)
}

/// Bob-side grinder: searches `sk_b` so `address(base_pk + sk_b·G)` matches.
#[derive(Clone)]
pub struct SplitKeyGrinder {
    base: PublicKey,
    coin: BitcoinLikeGrinder,
}

impl SplitKeyGrinder {
    pub fn new(base_pubkey_hex: &str, coin: BitcoinLikeGrinder) -> Result<Self, String> {
        Ok(Self {
            base: parse_public_key(base_pubkey_hex)?,
            coin,
        })
    }

    pub fn coin(&self) -> &BitcoinLikeGrinder {
        &self.coin
    }
}

impl ChainGrinder for SplitKeyGrinder {
    fn id(&self) -> &'static str {
        // Preserve the underlying coin id for UX (btc / rvn / …).
        self.coin.id
    }

    fn display_name(&self) -> &'static str {
        "Split-key (Bitcoin-family P2PKH)"
    }

    fn grind_attempt(&self) -> (String, GrindAttempt) {
        let secp = Secp256k1::new();
        loop {
            let (addr, attempt) = grind_secp256k1(|part| {
                let part_pk = part.public_key(&secp);
                match self.base.combine(&part_pk) {
                    Ok(sum) => p2pkh_address_from_pubkey(&sum, self.coin.version_byte),
                    Err(_) => String::new(), // retry rare invalid combine
                }
            });
            if !addr.is_empty() {
                return (addr, attempt);
            }
        }
    }

    fn finalize(&self, attempt: GrindAttempt) -> KeypairResult {
        let part_bytes = secret_from_attempt(attempt);
        let part = SecretKey::from_slice(&part_bytes).expect("valid part secret");
        let secp = Secp256k1::new();
        let sum = self
            .base
            .combine(&part.public_key(&secp))
            .expect("valid combine");
        let address = p2pkh_address_from_pubkey(&sum, self.coin.version_byte);
        KeypairResult {
            address: address.clone(),
            exports: vec![
                KeyExport {
                    label: "PrivkeyPart (hex)".into(),
                    value: hex::encode(part_bytes),
                    hint: Some(
                        "Send this part back to the key owner — it is NOT the final private key."
                            .into(),
                    ),
                },
                KeyExport {
                    label: "PrivkeyPart (WIF)".into(),
                    value: wif_encode(part_bytes, self.coin.wif_byte),
                    hint: Some("Same part as WIF for convenience".into()),
                },
                KeyExport {
                    label: "Vanity address".into(),
                    value: address,
                    hint: Some(
                        "Owner combines their secret + this part with `split-key combine`".into(),
                    ),
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
        self.coin.build_pattern(prefix, suffix, contains, exact)
    }

    fn expected_attempts(&self, pattern: &Pattern) -> f64 {
        self.coin.expected_attempts(pattern)
    }

    fn matches(&self, address: &str, pattern: &Pattern) -> bool {
        self.coin.matches(address, pattern)
    }

    fn supports_exact_case(&self) -> bool {
        true
    }

    fn pattern_hint(&self) -> &'static str {
        "Split-key grind: you only produce a key *part*. The owner combines it offline."
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chains::BitcoinLikeGrinder;

    #[test]
    fn roundtrip_gen_grind_combine() {
        let coin = BitcoinLikeGrinder::bitcoin();
        let alice = generate_split_keypair(&coin);
        let grinder = SplitKeyGrinder::new(&alice.public_key_hex, coin.clone()).unwrap();

        // Force a trivial pattern (lead only) so combine path is exercised quickly.
        let pattern = grinder.build_pattern(Some("1"), None, None, true).unwrap();
        let (addr, attempt) = loop {
            let (a, att) = grinder.grind_attempt();
            if grinder.matches(&a, &pattern) {
                break (a, att);
            }
        };
        let fin = grinder.finalize(attempt);
        assert_eq!(fin.address, addr);

        let part_hex = fin
            .exports
            .iter()
            .find(|e| e.label.starts_with("PrivkeyPart (hex)"))
            .unwrap()
            .value
            .clone();
        let alice_sk = parse_secret_key(&alice.private_key_hex).unwrap();
        let part_sk = parse_secret_key(&part_hex).unwrap();
        let combined = combine_split_keys(&alice_sk, &part_sk, &coin).unwrap();
        assert_eq!(combined.address, addr);
        // Combined priv must derive the vanity address.
        let sk = parse_secret_key(&combined.private_key_hex).unwrap();
        assert_eq!(p2pkh_address(&sk, coin.version_byte), addr);
    }

    #[test]
    fn rejects_bad_pubkey() {
        assert!(SplitKeyGrinder::new("00", BitcoinLikeGrinder::bitcoin()).is_err());
    }

    #[test]
    fn wif_roundtrip_and_rejects_bad_checksum() {
        let coin = BitcoinLikeGrinder::bitcoin();
        let alice = generate_split_keypair(&coin);
        let sk = parse_secret_key(&alice.private_key_wif).expect("valid WIF");
        assert_eq!(hex::encode(sk.secret_bytes()), alice.private_key_hex);

        // Flip one base58 char so checksum fails (keep length).
        let mut bad: Vec<u8> = alice.private_key_wif.bytes().collect();
        let last = bad.len() - 1;
        bad[last] = if bad[last] == b'1' { b'2' } else { b'1' };
        let bad_wif = String::from_utf8(bad).unwrap();
        let err = parse_secret_key(&bad_wif).unwrap_err();
        assert!(
            err.contains("checksum"),
            "expected checksum error, got: {err}"
        );
    }
}
