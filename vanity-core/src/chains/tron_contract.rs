//! Tron CREATE vanity — grind a deployer key so the contract it deploys with
//! its first transaction (nonce 0) lands on a matching `T…` address.
//!
//! Same RLP math as EVM CREATE (`keccak256(rlp([deployer, 0]))[12:]`), then
//! Tron base58check with version byte `0x41`.

use crate::chain::{ChainGrinder, GrindAttempt, KeyExport, KeypairResult};
use crate::pattern::Pattern;
use rand::rngs::OsRng;
use secp256k1::{Secp256k1, SecretKey};
use sha3::{Digest, Keccak256};

use super::base58_odds::Base58Layout;
use super::util::{
    base58_check_encode_raw, build_base58_pattern, matches_pattern, secret_from_attempt,
    BASE58_ALPHABET,
};

/// 0x41 + 20-byte hash + 4-byte checksum (same layout as wallet addresses).
const LAYOUT: Base58Layout = Base58Layout {
    version: &[0x41],
    random_len: 24,
    alphabet: BASE58_ALPHABET,
};

#[derive(Clone, Default)]
pub struct TronContractGrinder;

impl TronContractGrinder {
    fn deployer_eth_bytes(secret_key: &SecretKey) -> [u8; 20] {
        let secp = Secp256k1::new();
        let public_key = secret_key.public_key(&secp);
        let hash = Keccak256::digest(&public_key.serialize_uncompressed()[1..]);
        let mut out = [0u8; 20];
        out.copy_from_slice(&hash[12..]);
        out
    }

    fn deployer_tron_address(deployer: &[u8; 20]) -> String {
        let mut payload = vec![0x41u8];
        payload.extend_from_slice(deployer);
        base58_check_encode_raw(&payload)
    }

    /// RLP of `[address, 0]` — identical to EVM CREATE nonce 0, then Tron encode.
    fn contract_address(deployer: &[u8; 20]) -> String {
        let mut buf = [0u8; 23];
        buf[0] = 0xd6;
        buf[1] = 0x94;
        buf[2..22].copy_from_slice(deployer);
        buf[22] = 0x80;
        let hash = Keccak256::digest(buf);
        let mut payload = vec![0x41u8];
        payload.extend_from_slice(&hash[12..]);
        base58_check_encode_raw(&payload)
    }
}

impl ChainGrinder for TronContractGrinder {
    fn id(&self) -> &'static str {
        "trx-contract"
    }

    fn display_name(&self) -> &'static str {
        "Tron contract (CREATE, nonce 0)"
    }

    fn grind_attempt(&self) -> (String, GrindAttempt) {
        let secret_key = SecretKey::new(&mut OsRng);
        let address = Self::contract_address(&Self::deployer_eth_bytes(&secret_key));
        (address, GrindAttempt::Secret32(secret_key.secret_bytes()))
    }

    fn finalize(&self, attempt: GrindAttempt) -> KeypairResult {
        let secret_bytes = secret_from_attempt(attempt);
        let secret_key = SecretKey::from_slice(&secret_bytes).expect("valid secp256k1 secret");
        let deployer = Self::deployer_eth_bytes(&secret_key);

        KeypairResult {
            address: Self::contract_address(&deployer),
            exports: vec![
                KeyExport {
                    label: "Deployer Private Key (hex)".into(),
                    value: hex::encode(secret_bytes),
                    hint: Some(
                        "Deploy from this account as its FIRST transaction (nonce 0) — fund it, don't use it for anything else first"
                            .into(),
                    ),
                },
                KeyExport {
                    label: "Deployer Private Key (0x hex)".into(),
                    value: format!("0x{}", hex::encode(secret_bytes)),
                    hint: Some("TronLink / TronWeb / tronbox".into()),
                },
                KeyExport {
                    label: "Deployer Address".into(),
                    value: Self::deployer_tron_address(&deployer),
                    hint: Some("Send TRX here for energy/bandwidth before deploying".into()),
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
        LAYOUT.apply_lead("Tron contract", &mut pattern, "T")?;
        Ok(pattern)
    }

    fn expected_attempts(&self, pattern: &Pattern) -> f64 {
        LAYOUT.expected_attempts(pattern)
    }

    fn matches(&self, address: &str, pattern: &Pattern) -> bool {
        matches_pattern(address, pattern, false)
    }

    fn supports_exact_case(&self) -> bool {
        true
    }

    fn pattern_hint(&self) -> &'static str {
        "Base58 contract address — always starts with T (added for you). Deploy must be nonce 0."
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::ChainGrinder;

    #[test]
    fn create_nonce0_known_vector() {
        // Same deployer as EVM CREATE vector; Tron encodes the 20-byte CREATE hash with 0x41.
        let deployer: [u8; 20] = hex::decode("6ac7ea33f8831ea9dcc53393aaa88b25a785dbf0")
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(
            TronContractGrinder::contract_address(&deployer),
            "TUfsoPP1o7RDu7AGY3gQC2KA8zJ2XEnLTK"
        );
    }

    #[test]
    fn finalize_matches_grind_and_exports_deployer() {
        let g = TronContractGrinder;
        let (addr, attempt) = g.grind_attempt();
        let kp = g.finalize(attempt);
        assert_eq!(kp.address, addr);
        assert!(kp.address.starts_with('T'));
        let deployer = &kp.exports[2].value;
        assert!(deployer.starts_with('T'));
        assert_ne!(deployer, &kp.address);
    }
}
