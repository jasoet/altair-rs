//! `u64` encode/decode as 13-character Crockford strings.
//!
//! `encode_u64` always emits exactly 13 characters with leading zeros so
//! outputs are lexicographically sortable (the property ULID and CUID rely on).
//! `decode_u64` accepts any length up to 13 — leading-zero variants like `"5"`
//! and `"0000000000005"` decode to the same value.

use crate::bytes::normalize;
use crate::error::{Error, Result};

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const U64_ENCODED_LEN: usize = 13;

/// Encode `n` as a 13-character zero-padded Crockford Base32 string.
///
/// ```
/// assert_eq!(altair_base32::encode_u64(0),          "0000000000000");
/// assert_eq!(altair_base32::encode_u64(31),         "000000000000Z");
/// assert_eq!(altair_base32::encode_u64(32),         "0000000000010");
/// assert_eq!(altair_base32::encode_u64(u64::MAX),   "FZZZZZZZZZZZZ");
/// ```
#[must_use]
pub fn encode_u64(mut n: u64) -> String {
    let mut buf = [b'0'; U64_ENCODED_LEN];
    for slot in buf.iter_mut().rev() {
        *slot = ALPHABET[(n & 0b1_1111) as usize];
        n >>= 5;
    }
    String::from_utf8(buf.to_vec()).expect("ALPHABET contains only ASCII")
}

/// Decode a Crockford Base32 string into a `u64`.
///
/// Lenient (per crate-wide rules): case-insensitive; `I`/`L` → `1`, `O` → `0`;
/// hyphens stripped. Empty input decodes to `0`.
///
/// Returns [`Error::Overflow`] if the value exceeds `u64::MAX` (input length
/// `> 13` characters after normalization, unless all the leading extras are `0`).
///
/// ```
/// assert_eq!(altair_base32::decode_u64("").unwrap(), 0);
/// assert_eq!(altair_base32::decode_u64("0000000000005").unwrap(), 5);
/// assert_eq!(altair_base32::decode_u64("5").unwrap(), 5);
/// ```
pub fn decode_u64(text: &str) -> Result<u64> {
    let normalized = normalize(text)?;
    let bytes = normalized.as_bytes();

    let mut acc: u64 = 0;
    for (i, &ch) in bytes.iter().enumerate() {
        let value = char_value(ch);
        if bytes.len() - i > U64_ENCODED_LEN {
            // Still in the "expected to be all zeros" prefix
            if value != 0 {
                return Err(Error::Overflow);
            }
            continue;
        }
        acc = acc
            .checked_mul(32)
            .and_then(|v| v.checked_add(u64::from(value)))
            .ok_or(Error::Overflow)?;
    }
    Ok(acc)
}

/// Convert a normalized ASCII character into its 0-31 alphabet value.
///
/// Caller must have validated the character via [`normalize`] first.
fn char_value(ch: u8) -> u8 {
    match ch {
        b'0'..=b'9' => ch - b'0',
        b'A'..=b'H' => ch - b'A' + 10,
        b'J' => 18,
        b'K' => 19,
        b'M' => 20,
        b'N' => 21,
        b'P'..=b'T' => ch - b'P' + 22,
        b'V'..=b'Z' => ch - b'V' + 27,
        _ => unreachable!("normalize() should have rejected '{}'", ch as char),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn encode_zero_is_thirteen_zeros() {
        assert_eq!(encode_u64(0), "0000000000000");
    }

    #[test]
    fn encode_thirty_one_is_trailing_z() {
        assert_eq!(encode_u64(31), "000000000000Z");
    }

    #[test]
    fn encode_thirty_two_is_one_zero() {
        assert_eq!(encode_u64(32), "0000000000010");
    }

    #[test]
    fn encode_u64_max() {
        let s = encode_u64(u64::MAX);
        assert_eq!(s.len(), 13);
        assert_eq!(s, "FZZZZZZZZZZZZ");
    }

    #[test]
    fn round_trip_first_256() {
        for n in 0..256u64 {
            let encoded = encode_u64(n);
            assert_eq!(encoded.len(), 13);
            assert_eq!(decode_u64(&encoded).unwrap(), n);
        }
    }

    #[test]
    fn round_trip_random_values() {
        let values: [u64; 10] = [
            0,
            1,
            0xFF,
            0xFFFF,
            0xDEAD_BEEF,
            0xCAFE_F00D_5AAA_5555,
            1u64 << 32,
            1u64 << 60,
            (1u64 << 63) - 1,
            u64::MAX,
        ];
        for n in values {
            assert_eq!(decode_u64(&encode_u64(n)).unwrap(), n);
        }
    }

    #[test]
    fn decode_empty_is_zero() {
        assert_eq!(decode_u64("").unwrap(), 0);
    }

    #[test]
    fn decode_short_inputs_are_valid() {
        assert_eq!(decode_u64("0").unwrap(), 0);
        assert_eq!(decode_u64("5").unwrap(), 5);
        assert_eq!(decode_u64("Z").unwrap(), 31);
    }

    #[test]
    fn decode_leading_zeros_ignored() {
        assert_eq!(decode_u64("0000000000005").unwrap(), 5);
        assert_eq!(decode_u64("000005").unwrap(), 5);
    }

    #[test]
    fn decode_lenient_with_hyphens() {
        assert_eq!(decode_u64("FZZ-ZZZZ-ZZZZ-ZZ").unwrap(), u64::MAX);
    }

    #[test]
    fn decode_overflow_when_too_long_and_nonzero_prefix() {
        match decode_u64("10000000000000") {
            Err(Error::Overflow) => {}
            other => panic!("expected Overflow, got {other:?}"),
        }
    }

    #[test]
    fn decode_long_with_leading_zeros_ok() {
        assert_eq!(decode_u64("0FZZZZZZZZZZZZ").unwrap(), u64::MAX);
    }

    #[test]
    fn decode_overflow_when_value_exceeds_u64_max() {
        match decode_u64("ZZZZZZZZZZZZZZ") {
            Err(Error::Overflow) => {}
            other => panic!("expected Overflow, got {other:?}"),
        }
    }

    #[test]
    fn decode_invalid_char_propagates() {
        match decode_u64("ABC?DEF") {
            Err(Error::InvalidChar { ch: '?', .. }) => {}
            other => panic!("expected InvalidChar, got {other:?}"),
        }
    }
}
