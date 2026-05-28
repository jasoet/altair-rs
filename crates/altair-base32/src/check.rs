//! Mod-37,5 check digit (per Crockford spec, "Check Symbol" section).
//!
//! The check character is computed from the *byte* data — not from the
//! encoded string — using Horner's method `acc = (acc * 256 + byte) % 37`.
//! The remainder picks a character from the extended alphabet:
//!
//! ```text
//!   0-9, A-Z (standard 32 chars, values 0-31)
//!   *  →  32
//!   ~  →  33
//!   $  →  34
//!   =  →  35
//!   U  →  36
//! ```
//!
//! Crockford intentionally chose symbols that don't collide with the
//! standard alphabet so the check char is unambiguous.

use crate::bytes::{decode, encode, normalize};
use crate::error::{Error, Result};

const CHECK_ALPHABET: &[u8; 37] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ*~$=U";

/// Encode `data` and append the Mod-37,5 check digit.
///
/// Output is the standard [`crate::encode`] result with **one additional
/// character** appended — the check digit. The check alphabet includes
/// `*`, `~`, `$`, `=`, and `U` in addition to the standard 32 — be aware
/// that these are not URL-safe.
///
/// ```
/// let s = altair_base32::encode_with_check(b"hi");
/// // 'hi' encodes as "D1JPW", check of 'hi' (0x6869) is ... a valid char.
/// assert_eq!(s.len(), altair_base32::encode(b"hi").len() + 1);
/// ```
#[must_use]
pub fn encode_with_check(data: &[u8]) -> String {
    let mut out = encode(data);
    out.push(check_char_for(data));
    out
}

/// Decode a Crockford Base32 string that ends in a Mod-37,5 check digit.
///
/// Strips the last character (the check digit), decodes the body, then
/// verifies the check digit against the decoded bytes. Lenient decoding
/// (case-insensitive, I/L→1, O→0, hyphens stripped) applies to the body
/// in the same way as [`crate::decode`]; the check character itself is
/// case-sensitive (Crockford spec).
///
/// Returns:
/// - [`Error::Empty`] if the input is empty (need at least the check char)
/// - [`Error::CheckMismatch`] if the check digit doesn't match
/// - [`Error::InvalidChar`] if the body has any invalid character
/// - [`Error::InvalidChar`] if the trailing check char isn't in the
///   extended alphabet
pub fn decode_with_check(text: &str) -> Result<Vec<u8>> {
    let Some((last_pos, found)) = text.char_indices().next_back() else {
        return Err(Error::Empty);
    };
    let body = &text[..last_pos];

    let expected_value = check_value_from_char(found).ok_or(Error::InvalidChar {
        ch: found,
        position: last_pos,
    })?;

    let body_normalized = normalize(body)?;
    let bytes = decode(&body_normalized)?;

    let computed = checksum(&bytes);
    if computed == expected_value {
        Ok(bytes)
    } else {
        Err(Error::CheckMismatch {
            expected: CHECK_ALPHABET[computed as usize] as char,
            found,
        })
    }
}

/// Compute the Mod-37,5 checksum of a byte slice via Horner's method.
fn checksum(data: &[u8]) -> u8 {
    let mut acc: u32 = 0;
    for &b in data {
        acc = (acc * 256 + u32::from(b)) % 37;
    }
    // `acc` is in 0..37 here, which fits in u8.
    #[allow(clippy::cast_possible_truncation)]
    let result = acc as u8;
    result
}

fn check_char_for(data: &[u8]) -> char {
    CHECK_ALPHABET[checksum(data) as usize] as char
}

/// Inverse of [`check_char_for`] — maps an extended-alphabet character back
/// to its 0..37 value. Returns `None` for characters outside the extended
/// alphabet. Note: case-sensitive (Crockford spec).
fn check_value_from_char(ch: char) -> Option<u8> {
    CHECK_ALPHABET
        .iter()
        .position(|&c| c as char == ch)
        .and_then(|i| u8::try_from(i).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use pretty_assertions::assert_eq;

    #[test]
    fn check_alphabet_is_37_chars() {
        assert_eq!(CHECK_ALPHABET.len(), 37);
    }

    #[test]
    fn checksum_of_empty_is_zero() {
        assert_eq!(checksum(&[]), 0);
        assert_eq!(check_char_for(&[]), '0');
    }

    #[test]
    fn checksum_known_value() {
        // 'f' = 0x66 = 102; 102 % 37 = 28.
        // Index 28 in the check alphabet "0123456789ABCDEFGHJKMNPQRSTVWXYZ*~$=U":
        //   0-9 = indexes 0-9; A-H = 10-17; J=18, K=19; M=20, N=21;
        //   P-T = 22-26; V-Z = 27-31.  Index 28 = 'W'.
        assert_eq!(check_char_for(b"f"), 'W');
    }

    #[test]
    fn round_trip_succeeds() {
        for input in [&b""[..], b"f", b"fo", b"foo", b"hello world"] {
            let encoded = encode_with_check(input);
            let decoded = decode_with_check(&encoded).unwrap();
            assert_eq!(decoded, input);
        }
    }

    #[test]
    fn empty_input_is_empty_error() {
        match decode_with_check("") {
            Err(Error::Empty) => {}
            other => panic!("expected Empty, got {other:?}"),
        }
    }

    #[test]
    fn empty_payload_check_only_round_trips() {
        let encoded = encode_with_check(&[]);
        assert_eq!(encoded.len(), 1);
        assert_eq!(decode_with_check(&encoded).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn corrupting_check_char_yields_mismatch() {
        let mut encoded = encode_with_check(b"hello");
        let last_idx = encoded.len() - 1;
        let original = encoded.as_bytes()[last_idx];
        let replacement = if original == b'1' { b'2' } else { b'1' };
        encoded.replace_range(
            last_idx..encoded.len(),
            std::str::from_utf8(&[replacement]).unwrap(),
        );

        match decode_with_check(&encoded) {
            Err(Error::CheckMismatch { .. }) => {}
            other => panic!("expected CheckMismatch, got {other:?}"),
        }
    }

    #[test]
    fn corrupting_body_yields_mismatch() {
        let encoded = encode_with_check(b"hello");
        let first = encoded.as_bytes()[0];
        let replacement = if first == b'C' { b'D' } else { b'C' };
        let mut corrupted = encoded.clone();
        corrupted.replace_range(0..1, std::str::from_utf8(&[replacement]).unwrap());

        match decode_with_check(&corrupted) {
            Err(Error::CheckMismatch { .. }) => {}
            other => panic!("expected CheckMismatch, got {other:?}"),
        }
    }

    #[test]
    fn unknown_check_char_is_invalid() {
        match decode_with_check("CR?") {
            Err(Error::InvalidChar { ch: '?', .. }) => {}
            other => panic!("expected InvalidChar('?'), got {other:?}"),
        }
    }
}
