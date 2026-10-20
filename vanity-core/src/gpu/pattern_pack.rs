//! Pack [`Pattern`](crate::pattern::Pattern) into fixed GPU buffers.

use crate::pattern::Pattern;

/// Max literal nibbles / base58 chars we push to the device for prefix/suffix/contains.
pub const MAX_PAT: usize = 32;

/// Host↔OpenCL wire format. Field offsets are hard-coded in `kernels/*.cl`
/// (`plen@4`, `prefix@20`, …) — keep `repr(C)` and do not reorder fields.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GpuPattern {
    /// 0 = EVM hex (nibbles), 1 = Solana base58 bytes of the encoded address string.
    pub kind: u32,
    pub prefix_len: u32,
    pub suffix_len: u32,
    pub contains_len: u32,
    pub ignore_case: u32,
    pub prefix: [u8; MAX_PAT],
    pub suffix: [u8; MAX_PAT],
    pub contains: [u8; MAX_PAT],
}

impl GpuPattern {
    pub fn for_evm(pattern: &Pattern) -> Result<Self, String> {
        // EVM addresses are 0x + 40 hex chars; pattern stores without forcing 0x in match buffers.
        let prefix = strip0x(&pattern.prefix_match);
        let suffix = strip0x(&pattern.suffix_match);
        let contains = strip0x(&pattern.contains_match);
        Ok(Self {
            kind: 0,
            prefix_len: nibble_encode(&prefix, true)?,
            suffix_len: nibble_encode(&suffix, true)?,
            contains_len: nibble_encode(&contains, true)?,
            ignore_case: 1,
            prefix: nibble_bytes(&prefix, true)?,
            suffix: nibble_bytes(&suffix, true)?,
            contains: nibble_bytes(&contains, true)?,
        })
    }

    pub fn for_solana(pattern: &Pattern) -> Result<Self, String> {
        Ok(Self {
            kind: 1,
            prefix_len: ascii_len(&pattern.prefix_match)?,
            suffix_len: ascii_len(&pattern.suffix_match)?,
            contains_len: ascii_len(&pattern.contains_match)?,
            ignore_case: u32::from(pattern.ignore_case),
            prefix: ascii_bytes(&pattern.prefix_match)?,
            suffix: ascii_bytes(&pattern.suffix_match)?,
            contains: ascii_bytes(&pattern.contains_match)?,
        })
    }
}

fn strip0x(s: &str) -> String {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        rest.to_string()
    } else {
        t.to_string()
    }
}

fn ascii_len(s: &str) -> Result<u32, String> {
    if s.chars().count() > MAX_PAT {
        return Err(format!("GPU pattern longer than {MAX_PAT} chars"));
    }
    // Wildcards not supported on GPU MVP — reject `*`.
    if s.contains('*') {
        return Err(
            "GPU grind does not support '*' wildcards yet — use CPU or a literal pattern".into(),
        );
    }
    Ok(s.len() as u32)
}

fn ascii_bytes(s: &str) -> Result<[u8; MAX_PAT], String> {
    let _ = ascii_len(s)?;
    let mut out = [0u8; MAX_PAT];
    out[..s.len()].copy_from_slice(s.as_bytes());
    Ok(out)
}

/// Returns length in *nibbles* (hex chars).
fn nibble_encode(s: &str, _ignore_case: bool) -> Result<u32, String> {
    if s.contains('*') {
        return Err(
            "GPU grind does not support '*' wildcards yet — use CPU or a literal pattern".into(),
        );
    }
    if s.len() > MAX_PAT {
        return Err(format!("GPU hex pattern longer than {MAX_PAT} nibbles"));
    }
    for c in s.chars() {
        if !c.is_ascii_hexdigit() {
            return Err(format!("invalid hex pattern char '{c}'"));
        }
    }
    Ok(s.len() as u32)
}

fn nibble_bytes(s: &str, _ignore_case: bool) -> Result<[u8; MAX_PAT], String> {
    let _ = nibble_encode(s, true)?;
    let mut out = [0u8; MAX_PAT];
    for (i, c) in s.chars().enumerate() {
        out[i] = c.to_ascii_lowercase() as u8;
    }
    Ok(out)
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn gpu_pattern_wire_offsets() {
        assert_eq!(size_of::<GpuPattern>(), 116);
        assert_eq!(offset_of!(GpuPattern, kind), 0);
        assert_eq!(offset_of!(GpuPattern, prefix_len), 4);
        assert_eq!(offset_of!(GpuPattern, suffix_len), 8);
        assert_eq!(offset_of!(GpuPattern, contains_len), 12);
        assert_eq!(offset_of!(GpuPattern, ignore_case), 16);
        assert_eq!(offset_of!(GpuPattern, prefix), 20);
        assert_eq!(offset_of!(GpuPattern, suffix), 52);
        assert_eq!(offset_of!(GpuPattern, contains), 84);
    }
}
