//! Crockford Base32 byte encode/decode.
//!
//! `decode` and the private `normalize` helper handle the Crockford lenient
//! decoding rules:
//!
//! - Case-insensitive
//! - `I`/`L` → `1`, `O` → `0` (visual ambiguity normalization)
//! - Hyphens (`-`) stripped silently
//!
//! Anything else falls through to [`base32::decode`] with `Alphabet::Crockford`.

use crate::error::{Error, Result};
use base32::Alphabet;

const CROCKFORD: Alphabet = Alphabet::Crockford;

/// Encode `data` as a Crockford Base32 string.
///
/// Output uses only the canonical alphabet: `0-9A-Z` minus `I`, `L`, `O`, `U`.
/// No padding. No hyphenation.
///
/// ```
/// assert_eq!(altair_base32::encode(b"hi"), "D1JPW");
/// ```
#[must_use]
pub fn encode(data: &[u8]) -> String {
    base32::encode(CROCKFORD, data)
}

/// Decode a Crockford Base32 string.
///
/// Lenient: case-insensitive; `I`/`L` decoded as `1`; `O` decoded as `0`;
/// hyphens stripped silently.
///
/// ```
/// let plain = altair_base32::decode("D1JPW").unwrap();
/// assert_eq!(plain, b"hi");
/// ```
pub fn decode(text: &str) -> Result<Vec<u8>> {
    let normalized = normalize(text)?;
    // `base32::decode` returns `None` only on characters outside its alphabet —
    // we have already validated every character in `normalize`, so failure
    // here would be a bug. We map to a sentinel `Error::InvalidChar` at byte 0
    // defensively rather than panic.
    base32::decode(CROCKFORD, &normalized).ok_or(Error::InvalidChar {
        ch: '?',
        position: 0,
    })
}

/// Apply Crockford lenient rules and validate every character.
///
/// Returns the normalized ASCII string (uppercase, hyphens stripped, I/L→1,
/// O→0) ready for [`base32::decode`].
///
/// `position` in any returned `InvalidChar` is the byte offset in the
/// *original* `text` (before hyphen-stripping), so error messages point at
/// the character the user actually typed.
pub(crate) fn normalize(text: &str) -> Result<String> {
    let mut out = String::with_capacity(text.len());
    for (position, ch) in text.char_indices() {
        match ch {
            '-' => continue,
            '0' | 'O' | 'o' => out.push('0'),
            '1' | 'I' | 'i' | 'L' | 'l' => out.push('1'),
            '2'..='9' => out.push(ch),
            'a'..='z' if is_crockford_letter(ch.to_ascii_uppercase()) => {
                out.push(ch.to_ascii_uppercase());
            }
            'A'..='Z' if is_crockford_letter(ch) => out.push(ch),
            _ => return Err(Error::InvalidChar { ch, position }),
        }
    }
    Ok(out)
}

/// True if `ch` is an uppercase letter in the Crockford standard alphabet
/// (i.e. not `I`, `L`, `O`, or `U`). Caller must pass an ASCII uppercase letter.
fn is_crockford_letter(ch: char) -> bool {
    matches!(
        ch,
        'A'..='H' | 'J' | 'K' | 'M' | 'N' | 'P'..='T' | 'V'..='Z'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn encode_known_vectors() {
        // Canonical Crockford encodings — verified against the spec.
        assert_eq!(encode(b""), "");
        assert_eq!(encode(b"f"), "CR");
        assert_eq!(encode(b"fo"), "CSQG");
        assert_eq!(encode(b"foo"), "CSQPY");
        assert_eq!(encode(b"foob"), "CSQPYRG");
        assert_eq!(encode(b"fooba"), "CSQPYRK1");
        assert_eq!(encode(b"foobar"), "CSQPYRK1E8");
    }

    #[test]
    fn decode_canonical_round_trip() {
        for input in [
            &b""[..],
            b"f",
            b"fo",
            b"foo",
            b"foob",
            b"fooba",
            b"foobar",
            b"hello world",
        ] {
            assert_eq!(decode(&encode(input)).unwrap(), input);
        }
    }

    #[test]
    fn decode_lenient_lowercase() {
        assert_eq!(decode("csqpyrk1e8").unwrap(), b"foobar");
    }

    #[test]
    fn decode_lenient_i_l_o() {
        // 'I', 'L' should decode as 1; 'O' should decode as 0
        let canonical = encode(&[0x00, 0x01, 0x10, 0x11]);
        // Replace some '1's with 'I' / 'L' and '0's with 'O', should still decode the same
        let scrambled: String = canonical
            .chars()
            .map(|c| match c {
                '1' => 'L',
                '0' => 'O',
                other => other,
            })
            .collect();
        assert_eq!(decode(&scrambled).unwrap(), decode(&canonical).unwrap());
    }

    #[test]
    fn decode_strips_hyphens() {
        let plain = decode("CSQ-PYR-K1E-8").unwrap();
        assert_eq!(plain, b"foobar");
    }

    #[test]
    fn decode_invalid_char_reports_position() {
        match decode("CSQ?PYR") {
            Err(Error::InvalidChar { ch, position }) => {
                assert_eq!(ch, '?');
                assert_eq!(position, 3);
            }
            other => panic!("expected InvalidChar, got {other:?}"),
        }
    }

    #[test]
    fn decode_invalid_char_position_is_pre_hyphen_strip() {
        // "C-Q?" — '?' is at byte 3 in the *original* input.
        match decode("C-Q?") {
            Err(Error::InvalidChar { ch, position }) => {
                assert_eq!(ch, '?');
                assert_eq!(position, 3);
            }
            other => panic!("expected InvalidChar, got {other:?}"),
        }
    }

    #[test]
    fn decode_rejects_excluded_letters() {
        // 'U' is excluded from the Crockford standard alphabet (reserved as
        // the 37th check character). It must be rejected as invalid in decode.
        match decode("U") {
            Err(Error::InvalidChar { ch: 'U', .. }) => {}
            other => panic!("expected InvalidChar('U'), got {other:?}"),
        }
    }

    #[test]
    fn decode_empty_is_empty_vec() {
        assert_eq!(decode("").unwrap(), Vec::<u8>::new());
    }
}
