# altair-base32 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build, test, and publish `altair-base32` to crates.io as `v0.1.0` — Crockford Base32 encode/decode for bytes and u64, plus optional Mod-37,5 check digit.

**Architecture:** Single crate under `crates/altair-base32/`. Five Rust source files (lib.rs, error.rs, bytes.rs, u64.rs, check.rs) plus prelude. `bytes.rs` delegates to the `base32` crate (`Alphabet::Crockford`). `u64.rs` does manual base-32 arithmetic on a stack-allocated buffer (no allocations). `check.rs` implements Mod-37,5 via Horner's method directly on the input bytes. Lenient decoding (case-insensitive, I/L→1, O→0, hyphen-strip) is implemented once in `bytes::normalize` and reused.

**Tech Stack:**
- Rust 2024, MSRV 1.95 (inherit from workspace)
- `base32 = "0.5"` — for the byte encode/decode path
- `thiserror = "2"` (workspace) — for the `Error` enum
- No `tokio`, no `tracing` — pure synchronous encoding

---

## File Structure

```
crates/altair-base32/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs       # crate root: lints, mod declarations, re-exports
│   ├── error.rs     # Error enum + Result alias
│   ├── bytes.rs     # encode / decode + private normalize helper
│   ├── u64.rs       # encode_u64 / decode_u64
│   ├── check.rs     # encode_with_check / decode_with_check + Mod-37 checksum
│   └── prelude.rs   # re-export bundle
├── tests/
│   └── integration.rs
└── examples/
    └── basic.rs
```

Workspace edits:
- `Cargo.toml`: add `base32 = "0.5"` to `[workspace.dependencies]`; add `crates/altair-base32` to `members`
- `docs/porting-tracker.md`: move `altair-base32` from Deferred to In Progress (then Done after publish)
- `README.md`: add `altair-base32` row to crate table

---

## Phase 1: Crate Scaffold

Goal: empty but compilable crate registered in the workspace.

### Task 1.1: Add `base32` to workspace dependencies

**Files:**
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Add `base32` to `[workspace.dependencies]`**

In the root `Cargo.toml`, find the `[workspace.dependencies]` block and add (alphabetical placement near other encoding-ish deps, but the order doesn't matter):

```toml
# Encoding
base32 = "0.5"
```

- [ ] **Step 2: Verify workspace still parses**

Run: `cargo metadata --format-version=1 > /dev/null`
Expected: exit 0.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add base32 to workspace dependencies"
```

### Task 1.2: Create crate skeleton

**Files:**
- Create: `crates/altair-base32/Cargo.toml`
- Create: `crates/altair-base32/src/lib.rs`
- Create: `crates/altair-base32/README.md` (stub)
- Modify: `Cargo.toml` (workspace `members`)

- [ ] **Step 1: Create directories**

Run: `mkdir -p crates/altair-base32/src crates/altair-base32/tests crates/altair-base32/examples`

- [ ] **Step 2: Write `crates/altair-base32/Cargo.toml`**

```toml
[package]
name = "altair-base32"
description = "Crockford Base32 encoding with u64 helpers and Mod-37 check digit"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
homepage.workspace = true
readme = "README.md"
keywords = ["base32", "crockford", "encoding", "ulid", "id"]
categories = ["encoding"]

[dependencies]
base32 = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
pretty_assertions = { workspace = true }
anyhow = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 3: Write `crates/altair-base32/src/lib.rs`**

```rust
//! Crockford Base32 encode/decode with `u64` helpers and Mod-37,5 check digit.
//!
//! Implements [Crockford Base32](https://www.crockford.com/base32.html):
//!
//! - **Lenient decode** — case-insensitive; `I`/`L` decode as `1`, `O` decodes as `0`;
//!   hyphens (`-`) in encoded input are stripped silently.
//! - **`u64` helpers** — fixed 13-character, zero-padded output suitable for
//!   sortable ID generation (ULID/CUID style).
//! - **Mod-37,5 check digit** — opt-in via [`encode_with_check`] / [`decode_with_check`].
//!
//! # Example
//!
//! ```
//! use altair_base32::{encode, decode};
//!
//! let ciphertext = encode(b"hello world");
//! let plaintext = decode(&ciphertext).unwrap();
//! assert_eq!(plaintext, b"hello world");
//! ```
//!
//! # Example — `u64` for sortable IDs
//!
//! ```
//! use altair_base32::{encode_u64, decode_u64};
//!
//! let id = encode_u64(1_234_567_890);
//! assert_eq!(id.len(), 13);
//! assert_eq!(decode_u64(&id).unwrap(), 1_234_567_890);
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod bytes;
mod check;
mod error;
mod u64;

pub mod prelude;

pub use bytes::{decode, encode};
pub use check::{decode_with_check, encode_with_check};
pub use error::{Error, Result};
pub use u64::{decode_u64, encode_u64};
```

- [ ] **Step 4: Write `crates/altair-base32/README.md` (stub — expanded in Task 7.2)**

```markdown
# altair-base32

Crockford Base32 encoding with u64 helpers and Mod-37 check digit.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

(Full README added in a later task.)
```

- [ ] **Step 5: Add crate to workspace `members`**

In root `Cargo.toml`, find the `members = [ ... ]` list and add the new crate. The full list after this edit:

```toml
members = [
    "crates/altair-concurrent",
    "crates/altair-retry",
    "crates/altair-config",
    "crates/altair-otel",
    "crates/altair-base32",
]
```

- [ ] **Step 6: Verify the workspace parses**

Run: `cargo metadata --format-version=1 > /dev/null`
Expected: exit 0 (warning lines about missing `error.rs` etc. are tolerable here; cargo only parses manifests, not source).

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/altair-base32
git commit -m "feat(base32): scaffold altair-base32 crate"
```

---

## Phase 2: Error Type

Goal: every variant + Display rendering, locked in by tests.

### Task 2.1: Write `error.rs` with tests

**Files:**
- Create: `crates/altair-base32/src/error.rs`

- [ ] **Step 1: Write the file**

```rust
//! Crate-wide error type for `altair-base32`.

use thiserror::Error;

/// Errors returned by `altair-base32` decode operations.
#[derive(Debug, Error)]
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
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-base32 --lib error`
Expected: 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-base32/src/error.rs
git commit -m "feat(base32): add Error type and Result alias"
```

---

## Phase 3: Byte encode/decode + lenient normalize

Goal: working `encode`/`decode` with the full lenient decoding rule set, plus a private `normalize` helper that other modules will reuse.

### Task 3.1: Write `bytes.rs` with tests then impl

**Files:**
- Create: `crates/altair-base32/src/bytes.rs`

- [ ] **Step 1: Write the file**

```rust
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
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-base32 --lib bytes`
Expected: 9 tests pass.

> Sanity check on the canonical vectors: `encode(b"f") == "CR"` because `'f'` is `0x66 = 0110 0110` → first 5 bits `01100 = 12 = 'C'`, next 5 bits `110xx` where `xx` is zero-pad `11000 = 24 = 'R'`. If your local result differs from `"CR"`, do **not** "fix" the test until you've cross-checked with the `base32` crate's known vectors at the same version.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-base32/src/bytes.rs
git commit -m "feat(base32): byte encode/decode with lenient normalization"
```

---

## Phase 4: u64 helpers

Goal: fixed 13-character zero-padded `u64` encoding via stack-allocated manual base-32 arithmetic; decode accepts variable length up to 13 chars.

### Task 4.1: Write `u64.rs` with tests then impl

**Files:**
- Create: `crates/altair-base32/src/u64.rs`

- [ ] **Step 1: Write the file**

```rust
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
    // SAFETY: every byte in `buf` is an ASCII char from ALPHABET, so the
    // result is valid UTF-8. (We don't use `unsafe`; from_utf8 verifies.)
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
        // Once we've consumed 13 chars worth of bits, any further non-zero
        // characters would overflow.
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
        // Deterministic mix of large values — covers every bit position.
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
        // u64::MAX, hyphenated for readability
        assert_eq!(decode_u64("FZZ-ZZZZ-ZZZZ-ZZ").unwrap(), u64::MAX);
    }

    #[test]
    fn decode_overflow_when_too_long_and_nonzero_prefix() {
        // 14 chars, leading char nonzero → overflows
        match decode_u64("10000000000000") {
            Err(Error::Overflow) => {}
            other => panic!("expected Overflow, got {other:?}"),
        }
    }

    #[test]
    fn decode_long_with_leading_zeros_ok() {
        // 14 chars but leading char is '0' → fine, decodes the trailing 13
        assert_eq!(decode_u64("0FZZZZZZZZZZZZ").unwrap(), u64::MAX);
    }

    #[test]
    fn decode_overflow_when_value_exceeds_u64_max() {
        // 'G' = 16, so "G000000000000" = 16 * 32^12 which fits, but
        // "GZZZZZZZZZZZZ" = (16 * 32^12) + (32^12 - 1) — let's verify
        // a clearly-over case: "FZZZZZZZZZZZZ" is u64::MAX, anything one higher
        // overflows. Adding any non-zero digit at position 14 would do it; we
        // already tested that. Try "ZZZZZZZZZZZZZZ" (14 chars all Z).
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
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-base32 --lib u64`
Expected: 13 tests pass.

> Note on the `b"0123456789ABCDEFGHJKMNPQRSTVWXYZ"` literal: the offsets in `char_value` (`b'A'..='H'` → +10, `b'J'` → 18, etc.) must match exactly. If a test fails on a specific letter, recompute the offset rather than guessing.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-base32/src/u64.rs
git commit -m "feat(base32): u64 encode/decode (13-char zero-padded, sortable)"
```

---

## Phase 5: Check digit

Goal: Mod-37,5 implementation, plus `encode_with_check` / `decode_with_check`.

### Task 5.1: Write `check.rs` with tests then impl

**Files:**
- Create: `crates/altair-base32/src/check.rs`

- [ ] **Step 1: Write the file**

```rust
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
    if text.is_empty() {
        return Err(Error::Empty);
    }
    // The check character is the last *char* of the input. Hyphens at the
    // very end would be stripped by normalize; we don't allow trailing
    // hyphens between body and check char because the position semantics
    // would be confusing — pull the last char as-is.
    let mut chars = text.char_indices();
    let last = chars.next_back().expect("non-empty");
    let (last_pos, found) = last;
    let body = &text[..last_pos];

    let expected_value = check_value_from_char(found).ok_or(Error::InvalidChar {
        ch: found,
        position: last_pos,
    })?;

    // Decode the body via the standard byte path (lenient).
    let body_normalized = normalize(body)?;
    let bytes = base32::decode(base32::Alphabet::Crockford, &body_normalized).ok_or(
        Error::InvalidChar {
            ch: '?',
            position: 0,
        },
    )?;

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
    u8::try_from(acc).expect("0 <= acc < 37")
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
        .map(|i| u8::try_from(i).expect("alphabet has 37 entries"))
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
        // Flip the check char to something else in the extended alphabet
        let last_idx = encoded.len() - 1;
        let original = encoded.as_bytes()[last_idx];
        // pick a different valid check char
        let replacement = if original == b'1' { b'2' } else { b'1' };
        // SAFETY-free: we replace one ASCII byte with another ASCII byte
        encoded.replace_range(last_idx..encoded.len(), std::str::from_utf8(&[replacement]).unwrap());

        match decode_with_check(&encoded) {
            Err(Error::CheckMismatch { .. }) => {}
            other => panic!("expected CheckMismatch, got {other:?}"),
        }
    }

    #[test]
    fn corrupting_body_yields_mismatch() {
        let encoded = encode_with_check(b"hello");
        // Replace the first body char with a different valid char
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
        // '?' is not in the check alphabet
        match decode_with_check("CR?") {
            Err(Error::InvalidChar { ch: '?', .. }) => {}
            other => panic!("expected InvalidChar('?'), got {other:?}"),
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-base32 --lib check`
Expected: 9 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-base32/src/check.rs
git commit -m "feat(base32): Mod-37,5 check digit (encode_with_check / decode_with_check)"
```

---

## Phase 6: Prelude module

Goal: one-import convenience module.

### Task 6.1: Write `prelude.rs`

**Files:**
- Create: `crates/altair-base32/src/prelude.rs`

- [ ] **Step 1: Write the file**

```rust
//! Common imports for users of this crate.
//!
//! ```
//! use altair_base32::prelude::*;
//!
//! let id = encode_u64(42);
//! assert_eq!(decode_u64(&id).unwrap(), 42);
//! ```

pub use crate::{
    Error, Result, decode, decode_u64, decode_with_check, encode, encode_u64, encode_with_check,
};
```

- [ ] **Step 2: Run doc test**

Run: `cargo test -p altair-base32 --doc`
Expected: all doc tests pass (lib.rs examples + prelude.rs example + per-function ones).

- [ ] **Step 3: Commit**

```bash
git add crates/altair-base32/src/prelude.rs
git commit -m "feat(base32): add prelude module"
```

---

## Phase 7: Integration tests, example, README

### Task 7.1: Integration tests

**Files:**
- Create: `crates/altair-base32/tests/integration.rs`

- [ ] **Step 1: Write the file**

```rust
//! End-to-end behavior tests for altair-base32.

use altair_base32::prelude::*;
use pretty_assertions::assert_eq;

#[test]
fn round_trip_1kb_random_data() {
    // Deterministic "random" data — fill with a simple PRNG so the test is reproducible.
    let mut buf = Vec::with_capacity(1024);
    let mut state: u32 = 0xDEAD_BEEF;
    for _ in 0..1024 {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12345);
        buf.push((state >> 16) as u8);
    }

    let encoded = encode(&buf);
    let decoded = decode(&encoded).unwrap();
    assert_eq!(decoded, buf);
}

#[test]
fn round_trip_1kb_with_check() {
    let mut buf = Vec::with_capacity(1024);
    let mut state: u32 = 0xCAFE_F00D;
    for _ in 0..1024 {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12345);
        buf.push((state >> 16) as u8);
    }

    let encoded = encode_with_check(&buf);
    let decoded = decode_with_check(&encoded).unwrap();
    assert_eq!(decoded, buf);
}

#[test]
fn ulid_style_combined_id() {
    // ULID convention: 48 bits of timestamp + 80 bits of randomness.
    // We split as: encode_u64(timestamp_ms) || encode(random_bytes)
    let timestamp: u64 = 1_700_000_000_000;
    let random_bytes: [u8; 10] = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22, 0x33, 0x44];

    let ts_part = encode_u64(timestamp);
    let rand_part = encode(&random_bytes);
    let combined = format!("{ts_part}{rand_part}");

    // Split back at known offset and decode each half independently.
    let ts_decoded = decode_u64(&combined[..13]).unwrap();
    let rand_decoded = decode(&combined[13..]).unwrap();
    assert_eq!(ts_decoded, timestamp);
    assert_eq!(rand_decoded, random_bytes);
}

#[test]
fn lenient_decoding_through_full_api() {
    // A user typing the encoded string by hand might confuse 1/I/L and 0/O,
    // mix case, and add hyphens for readability. Decoded result must match.
    let canonical = encode(b"Hello, world!");
    let user_typed = {
        let mut s: String = canonical
            .chars()
            .map(|c| match c {
                '1' => 'L',
                '0' => 'O',
                other => other.to_ascii_lowercase(),
            })
            .collect();
        // Add hyphens every 4 chars
        let mut hyphenated = String::new();
        for (i, ch) in s.chars().enumerate() {
            if i > 0 && i % 4 == 0 {
                hyphenated.push('-');
            }
            hyphenated.push(ch);
        }
        s = hyphenated;
        s
    };
    assert_eq!(decode(&user_typed).unwrap(), b"Hello, world!");
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p altair-base32 --tests`
Expected: 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-base32/tests/integration.rs
git commit -m "test(base32): integration tests for round-trip, ULID-style IDs, lenient decoding"
```

### Task 7.2: Example binary

**Files:**
- Create: `crates/altair-base32/examples/basic.rs`

- [ ] **Step 1: Write the file**

```rust
//! Run with: `cargo run --example basic -p altair-base32`

use altair_base32::prelude::*;

fn main() -> anyhow::Result<()> {
    // 1. Byte encoding
    let plaintext = b"hello world";
    let encoded = encode(plaintext);
    println!("encode('hello world') = {encoded}");
    println!("decode back            = {:?}", String::from_utf8(decode(&encoded)?)?);
    println!();

    // 2. u64 encoding for sortable IDs
    let timestamp_ms: u64 = 1_700_000_000_000;
    let id = encode_u64(timestamp_ms);
    println!("encode_u64({timestamp_ms}) = {id}");
    println!("decode_u64 back          = {}", decode_u64(&id)?);
    println!();

    // 3. With check digit for integrity verification
    let serial = b"PROD-2026-X";
    let with_check = encode_with_check(serial);
    println!("encode_with_check        = {with_check}");
    let recovered = decode_with_check(&with_check)?;
    println!(
        "decode_with_check back   = {:?}",
        String::from_utf8(recovered)?
    );

    // 4. Lenient decoding tolerates user typos
    let canonical = encode(b"hi there");
    let typo = canonical.replace('1', "L").replace('0', "O").to_lowercase();
    println!();
    println!("canonical = {canonical}");
    println!("typo'd    = {typo}");
    println!("both decode to: {:?}", String::from_utf8(decode(&typo)?)?);

    Ok(())
}
```

- [ ] **Step 2: Build and run**

Run: `cargo run -p altair-base32 --example basic`
Expected: prints four sections, exits 0.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-base32/examples/basic.rs
git commit -m "docs(base32): basic usage example"
```

### Task 7.3: README

**Files:**
- Modify: `crates/altair-base32/README.md`

- [ ] **Step 1: Replace the README with the full version**

```markdown
# altair-base32

[Crockford Base32](https://www.crockford.com/base32.html) encoding for Rust — byte slices, `u64` IDs, and optional Mod-37 check digit.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Why Crockford?

Standard RFC 4648 Base32 was designed for binary-to-text safety but isn't comfortable for humans to type or read aloud. Crockford's variant:

- Excludes `I`, `L`, `O`, `U` from the alphabet to avoid visual ambiguity with `1`, `0`
- Allows lenient decoding (`I`/`L` → `1`, `O` → `0`) so user typos still decode correctly
- Supports an optional check character for data-integrity verification
- Tolerates hyphens in encoded input for readability (`XXXX-YYYY-ZZZZ`)

## Add to your project

```bash
cargo add altair-base32
```

## Byte encoding

```rust,no_run
use altair_base32::{encode, decode};

let cipher = encode(b"hello world");
let plain = decode(&cipher).unwrap();
assert_eq!(plain, b"hello world");
```

## `u64` IDs — sortable, fixed-length

```rust,no_run
use altair_base32::{encode_u64, decode_u64};

let id = encode_u64(1_700_000_000_000);
assert_eq!(id.len(), 13);    // always 13 chars, zero-padded
assert_eq!(decode_u64(&id).unwrap(), 1_700_000_000_000);
```

Outputs are lexicographically sortable in the same order as the underlying integers — the property ULID and CUID rely on for time-ordered IDs.

## Mod-37 check digit

```rust,no_run
use altair_base32::{encode_with_check, decode_with_check};

let serial = b"PROD-XYZ-2026";
let with_check = encode_with_check(serial);

// Any single-character corruption is detected
let recovered = decode_with_check(&with_check).unwrap();
assert_eq!(recovered, serial);
```

> **URL safety:** the check character may be `*`, `~`, `$`, `=`, or `U` (the extended alphabet). If you need URL-safe output, URL-encode the result or use plain `encode` without the check digit.

## Lenient decoding

All `decode*` functions tolerate Crockford-spec input variations:

- Case-insensitive
- `I`/`L`/`i`/`l` → `1`
- `O`/`o` → `0`
- Hyphens (`-`) stripped silently

```rust,no_run
# use altair_base32::decode;
// All four of these decode to the same bytes:
assert_eq!(decode("CSQPYRK1E8").unwrap(), b"foobar");
assert_eq!(decode("csqpyrk1e8").unwrap(), b"foobar");
assert_eq!(decode("CSQ-PYR-K1E-8").unwrap(), b"foobar");
assert_eq!(decode("CSQPYRKLE8").unwrap(), b"foobar");  // L instead of 1
```

## Error reference

| Variant | When |
|---|---|
| `Error::InvalidChar` | Decode hit a character not in the Crockford alphabet (after lenient normalization) |
| `Error::CheckMismatch` | `decode_with_check` — check digit didn't match the decoded body |
| `Error::Overflow` | `decode_u64` — input represents a value > `u64::MAX` |
| `Error::Empty` | `decode_with_check` — input is empty (needs at least the check char) |

## License

[MIT](../../LICENSE)
```

- [ ] **Step 2: Verify doc tests pass**

Run: `cargo test -p altair-base32 --doc`
Expected: all doc tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-base32/README.md
git commit -m "docs(base32): complete README with examples and error reference"
```

---

## Phase 8: Tracker updates and final CI

### Task 8.1: Update porting tracker

**Files:**
- Modify: `docs/porting-tracker.md`

- [ ] **Step 1: Move the `altair-base32` row out of "Awaiting Demand" into a new published row**

Find the row:

```markdown
| `base32` | `altair-base32` | 💤 Deferred | (custom impl, possibly `data-encoding` for primitives) | Crockford Base32 — small focused crate |
```

Replace its **Status** column to `✅ Done` and update the **Underlying libs** to reflect the chosen approach:

```markdown
| `base32` | `altair-base32` | ✅ Done | `base32` (Crockford alphabet) + in-crate Mod-37 check | Crockford Base32 — small focused crate |
```

Also update the "v0.1.x" release notes section at the top of the file to add a new bullet:

```markdown
- **`altair-base32` 0.1.0** (date TBD on publish) — Crockford Base32 encode/decode for bytes and u64, plus Mod-37 check digit. Lenient decoding per spec.
```

- [ ] **Step 2: Commit**

```bash
git add docs/porting-tracker.md
git commit -m "docs: add altair-base32 to porting tracker"
```

### Task 8.2: Add crate to root README

**Files:**
- Modify: `README.md` (workspace root)

- [ ] **Step 1: Add a row to the crate table**

After the `altair-concurrent` row, add:

```markdown
| [`altair-base32`](crates/altair-base32) | Crockford Base32 — bytes, u64 IDs, optional check digit | [![crate](https://img.shields.io/crates/v/altair-base32.svg)](https://crates.io/crates/altair-base32) |
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: list altair-base32 in workspace README"
```

### Task 8.3: Full workspace CI check

- [ ] **Step 1: Run formatter, clippy, all tests, doc build**

Run:
```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --no-deps --all-features
```

Expected: all four commands exit 0. If clippy flags something in the new crate, fix it before proceeding — don't paper over with `#[allow]` unless the original error is genuinely incorrect for our context.

- [ ] **Step 2: Verify `cargo publish --dry-run`**

Run: `cargo publish --dry-run -p altair-base32`
Expected: "Uploading altair-base32 v0.1.0", "warning: aborting upload due to dry run".

- [ ] **Step 3: Commit any clippy/fmt fixes (if any)**

```bash
git add -p
git commit -m "fix(base32): satisfy clippy/fmt"
```

(Skip this step if there's nothing to commit.)

---

## Phase 9: Push, PR, publish

### Task 9.1: Push branch and open PR

- [ ] **Step 1: Push**

```bash
git push -u origin feat/altair-base32
```

- [ ] **Step 2: Open PR**

```bash
gh pr create --title "feat(base32): add altair-base32 crate" --body "$(cat <<'EOF'
## Summary

Adds the fifth crate to the workspace: \`altair-base32\` — Crockford Base32 encoding for Rust.

- \`encode\` / \`decode\` — byte slices, wraps \`base32\` crate (\`Alphabet::Crockford\`)
- \`encode_u64\` / \`decode_u64\` — 13-char zero-padded ULID-style IDs
- \`encode_with_check\` / \`decode_with_check\` — Mod-37,5 check digit
- Lenient decoding per spec: I/L→1, O→0, case-insensitive, hyphens stripped

## Test plan

- [x] \`cargo test --workspace\` — 30+ new tests pass
- [x] \`cargo clippy --workspace --all-targets --all-features -- -D warnings\` clean
- [x] \`cargo fmt --all --check\` clean
- [x] \`cargo doc --workspace --no-deps --all-features\` clean
- [x] \`cargo publish --dry-run -p altair-base32\` clean
- [ ] CI passes on this PR

## Release implication

Adds one new crate at 0.1.0 — existing crates not affected. Workspace version stays at 0.1.2.
EOF
)"
```

- [ ] **Step 3: Wait for CI and merge**

```bash
gh pr checks <pr-number> --watch
gh pr merge <pr-number> --squash --delete-branch
```

### Task 9.2: First publish via release-plz

On merge, the release workflow runs automatically. release-plz will:

1. Detect `altair-base32` is new on main but not on crates.io.
2. Open a release PR proposing `altair-base32 v0.1.0` (other crates unchanged).
3. On merge of that PR, the workflow publishes the crate.

- [ ] **Step 1: Watch the release workflow**

```bash
gh run watch
```

- [ ] **Step 2: Inspect the release-plz PR**

The release PR should add `crates/altair-base32/CHANGELOG.md` only — no changes to existing crates.

- [ ] **Step 3: Merge the release PR**

```bash
gh pr merge <release-pr-number> --squash --delete-branch
```

- [ ] **Step 4: Verify on crates.io**

```bash
curl -s -H 'User-Agent: altair-rs (jasoet87@gmail.com)' \
  https://crates.io/api/v1/crates/altair-base32 | jq -r .crate.max_version
```

Expected: `0.1.0`.

### Task 9.3: Final tracker update

**Files:**
- Modify: `docs/porting-tracker.md`

- [ ] **Step 1: Replace "date TBD on publish" with the actual date**

Find the bullet in the v0.1.x release notes section and update the date.

- [ ] **Step 2: Commit and push**

```bash
git checkout main && git pull
git checkout -b docs/base32-published
# (edit the file)
git commit -am "docs: record altair-base32 0.1.0 publish date"
gh pr create --title "docs: record altair-base32 0.1.0 publish date" --body "Trivial tracker update."
```

---

## Self-Review

### Spec Coverage Check

| Spec section | Implemented in task |
|---|---|
| §1 Overview | Plan header + Phase 7 README |
| §2 Decisions (workspace deps, naming, MSRV, API style) | Tasks 1.1, 1.2 |
| §3.1 File layout | Tasks 1.2, 2.1, 3.1, 4.1, 5.1, 6.1, 7.1, 7.2, 7.3 |
| §3.2 Module responsibilities | Tasks 2.1, 3.1, 4.1, 5.1 — each module is one file, sole owner of its concern |
| §3.3 Public API surface | Task 1.2 (lib.rs re-exports) + per-feature tasks |
| §3.4 Error model | Task 2.1 — every variant tested |
| §4.1 Lenient decoding rules | Task 3.1 (`normalize` function) + tests `decode_lenient_lowercase`, `decode_lenient_i_l_o`, `decode_strips_hyphens` |
| §4.2 u64 format (13 chars, zero-padded, sortable) | Task 4.1 — tests `encode_zero_is_thirteen_zeros`, `encode_u64_max` |
| §4.3 Mod-37 check digit | Task 5.1 — `checksum`, `CHECK_ALPHABET`, round-trip and mismatch tests |
| §5 Testing strategy (unit, integration, doc, example, ≥90% cov) | Tasks 2.1, 3.1, 4.1, 5.1 (unit); 7.1 (integration); 7.2 (example); doc tests embedded in source |
| §6 Cross-crate (no otel, prelude limited, no base32 re-export) | Task 1.2 lib.rs has no `pub use base32`; Task 6.1 prelude limited |
| §7 Out of scope | Plan only adds documented items |
| §8 Risks (base32 stability, position semantics) | Task 1.1 pins via workspace.dependencies; Task 2.1 error doc-comment notes original-input semantics |
| §9 Versioning | Task 1.2 `version.workspace = true` inherits 0.1.2 baseline; release-plz tags 0.1.0 for new crate |

### Placeholder Scan

No "TBD", "TODO", "fill in later", or unhandled edge cases. The one explicit "date TBD on publish" in the porting tracker is resolved in Task 9.3.

### Type Consistency

- `Error` enum variants used in tests match the definition in Task 2.1: `InvalidChar { ch, position }`, `CheckMismatch { expected, found }`, `Overflow`, `Empty`. ✓
- `Result<T>` alias used consistently throughout (Task 2.1 defines it; later tasks `use crate::error::Result`). ✓
- `normalize` is `pub(crate)` (Task 3.1) and used by `u64::decode_u64` (Task 4.1) and `check::decode_with_check` (Task 5.1). ✓
- `CHECK_ALPHABET` is `&[u8; 37]` consistently (Task 5.1). ✓
- `encode_u64` returns `String` of length exactly 13 — verified in tests `encode_zero_is_thirteen_zeros`, `encode_u64_max`. ✓

No drift detected.

---

## Execution Handoff

**Plan complete and saved to `docs/plans/2026-05-28-altair-base32-implementation.md`. Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** — execute tasks in this session via executing-plans, batch with checkpoints

Pick when ready to start implementation.
