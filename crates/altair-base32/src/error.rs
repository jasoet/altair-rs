//! Crate-wide error type for `altair-base32`.

use thiserror::Error;

/// Errors returned by `altair-base32` decode operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// Input contains a character not in the Crockford alphabet (after lenient
    /// normalization for `I`/`L`/`O` and hyphen-stripping).
    ///
    /// `position` is the byte offset in the original input string (before
    /// hyphen-stripping), so error messages point at the character the user
    /// actually typed.
    #[error("invalid Crockford character {ch:?} at position {position}")]
    InvalidChar {
        /// The offending character.
        ch: char,
        /// Byte offset in the original input.
        position: usize,
    },

    /// Check digit at the end of input didn't match the computed value.
    /// Returned only by [`crate::decode_with_check`].
    #[error("check digit mismatch: expected '{expected}', found '{found}'")]
    CheckMismatch {
        /// Check character the data should have ended with.
        expected: char,
        /// Check character the input ended with.
        found: char,
    },

    /// Decoded value overflows `u64`. Returned only by [`crate::decode_u64`].
    #[error("decoded value overflows u64")]
    Overflow,

    /// Input was empty where empty isn't valid (e.g. [`crate::decode_with_check`]
    /// requires at least the check character).
    #[error("input is empty")]
    Empty,
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_char_renders_position() {
        let e = Error::InvalidChar {
            ch: '?',
            position: 5,
        };
        let s = e.to_string();
        assert!(s.contains("'?'"));
        assert!(s.contains("position 5"));
    }

    #[test]
    fn check_mismatch_renders_both_chars() {
        let e = Error::CheckMismatch {
            expected: 'A',
            found: 'B',
        };
        let s = e.to_string();
        assert!(s.contains("'A'"));
        assert!(s.contains("'B'"));
    }

    #[test]
    fn overflow_renders() {
        assert_eq!(Error::Overflow.to_string(), "decoded value overflows u64");
    }

    #[test]
    fn empty_renders() {
        assert_eq!(Error::Empty.to_string(), "input is empty");
    }
}
