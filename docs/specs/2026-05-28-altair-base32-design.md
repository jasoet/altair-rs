# altair-base32 — Design

**Date:** 2026-05-28
**Status:** Draft — awaiting review before implementation planning
**Author:** Jasoet
**Spec type:** Brainstorming output → input to writing-plans

---

## 1. Overview

`altair-base32` is a small focused crate for [Crockford Base32](https://www.crockford.com/base32.html) encoding. It provides byte-slice encode/decode, fixed-length `u64` helpers (ULID-style 13-character output), and optional Mod-37,5 check-digit support — all under a unified `Result<T, Error>` surface.

**One-line product goal:** "Crockford Base32 for Rust, the way you'd want to use it."

The crate wraps the popular [`base32`](https://crates.io/crates/base32) crate (22M downloads, `Alphabet::Crockford` variant) for the byte path and implements the Crockford-specific features (u64 helpers, Mod-37,5 check digit, hyphen-stripping on decode) directly. Lenient decoding (case-insensitive, I/L→1, O→0, hyphen-tolerant) is applied uniformly per the Crockford spec.

## 2. Decisions Locked

| Decision | Choice |
|---|---|
| Scope | Bytes encode/decode, `u64` encode/decode, with-check variants on bytes |
| Implementation strategy | Wrap [`base32`](https://crates.io/crates/base32) for the byte path; implement u64 and check digit ourselves |
| Crate name | `altair-base32` (verified available on crates.io 2026-05-28) |
| API style | Free functions in the crate namespace (no struct/builder — single algorithm, no config) |
| `u64` output format | Fixed 13 chars, zero-padded (ULID/CUID-style; sortable) |
| Lenient decoding | Always on per Crockford spec (case-insensitive, I/L→1, O→0, hyphens stripped) |
| Check digit | Mod-37,5 per Crockford spec; opt-in via `_with_check` variants |
| `encode_u64_with_check` | **Not included** — users wanting check on a `u64` convert to 8 bytes first |
| Error type | Single `thiserror` enum: `InvalidChar { ch, position }`, `CheckMismatch`, `Overflow`, `Empty` |
| Dependencies | `base32`, `thiserror` (workspace) — no `tracing`, no `tokio` |
| Edition / MSRV | Inherit from workspace (Edition 2024, Rust 1.95) |

## 3. Architecture

### 3.1 File layout

```
crates/altair-base32/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs         # crate root: lints, re-exports, prelude module
│   ├── error.rs       # Error enum + Result alias (thiserror)
│   ├── bytes.rs       # encode / decode  (wraps base32::Alphabet::Crockford)
│   ├── u64.rs         # encode_u64 / decode_u64  (13-char zero-padded)
│   ├── check.rs       # Mod-37,5 helpers + encode_with_check / decode_with_check
│   └── prelude.rs     # one-import bundle: 6 functions + Error + Result
├── tests/
│   └── integration.rs # Crockford spec test vectors + lenient decoding + round-trip
└── examples/
    └── basic.rs       # ID generation + with-check round-trip
```

### 3.2 Module responsibilities

- **`error.rs`** — sole owner of the `Error` enum and `Result<T>` alias. Other modules use them and never define their own.
- **`bytes.rs`** — the only place the `base32` crate is touched. Provides `normalize(text: &str) -> Cow<'_, str>` helper that handles I/L→1, O→0, hyphen-strip, case-fold before delegating to `base32::decode`. Used by `u64.rs` and `check.rs` for consistent leniency.
- **`u64.rs`** — converts `u64` ↔ 13-char Crockford string by manual base-32 arithmetic (faster than allocating a `Vec<u8>` then encoding). Zero-padded.
- **`check.rs`** — Mod-37,5 algorithm: divide the integer interpretation of the data by 37, the remainder picks a character from the extended alphabet `0..9A..Z*~$=U`. Appended to the byte encoding; verified and stripped on decode.

### 3.3 Public API

```rust
// crate root re-exports
pub use error::{Error, Result};
pub use bytes::{encode, decode};
pub use u64::{encode_u64, decode_u64};
pub use check::{encode_with_check, decode_with_check};

pub mod prelude;
```

Function signatures:

```rust
pub fn encode(data: &[u8]) -> String;
pub fn decode(text: &str) -> Result<Vec<u8>>;

pub fn encode_u64(n: u64) -> String;        // always 13 chars, zero-padded
pub fn decode_u64(text: &str) -> Result<u64>;

pub fn encode_with_check(data: &[u8]) -> String;
pub fn decode_with_check(text: &str) -> Result<Vec<u8>>;
```

### 3.4 Error model

```rust
#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid Crockford character {ch:?} at position {position}")]
    InvalidChar { ch: char, position: usize },

    #[error("check digit mismatch: expected '{expected}', found '{found}'")]
    CheckMismatch { expected: char, found: char },

    #[error("decoded value overflows u64")]
    Overflow,

    #[error("input is empty")]
    Empty,
}
```

**Empty-input behavior:**
- `decode("")` → `Ok(vec![])`
- `decode_u64("")` → `Ok(0)`. `decode_u64` accepts variable-length input — `"0"`, `"5"`, `"0000000000000"`, `"5"` are all valid; only the integer value matters. Length > 13 is `Err(Overflow)` unless the leading characters are zero.
- `decode_with_check("")` → `Err(Empty)` (needs at least the check char)
- `encode_with_check(&[])` → `"<check_of_empty>"` — single character (the check digit for an empty payload). Roundtrips through `decode_with_check` to `Ok(vec![])`.

**`position`** is the byte offset in the original input string (before hyphen-stripping) so error messages point users at the actual character they typed.

## 4. Behaviour Details

### 4.1 Lenient decoding (applied to every `decode*` function)

| Input character | Treated as |
|---|---|
| `0`, `O`, `o` | `0` |
| `1`, `I`, `i`, `L`, `l` | `1` |
| `2`–`9`, `A`–`H`, `J`, `K`, `M`, `N`, `P`–`T`, `V`–`Z` (any case) | their canonical value |
| `-` (hyphen) | stripped |
| anything else | `Error::InvalidChar { ch, position }` |

### 4.2 `u64` encoding format

`u64::MAX = 18_446_744_073_709_551_615` encodes to 13 Crockford characters (since `32^13 > 2^64`). Smaller values are zero-padded to 13 chars so outputs are lexicographically sortable in the natural order — the property ULID and CUID rely on.

```text
encode_u64(0)         == "0000000000000"
encode_u64(31)        == "000000000000Z"
encode_u64(32)        == "0000000000010"
encode_u64(u64::MAX)  == "FZZZZZZZZZZZZ"
```

### 4.3 Check digit (Mod-37,5)

Per Crockford spec section "Check Symbol":

1. Treat the encoded data as a big integer (most-significant character first).
2. Take `value mod 37`.
3. Map the remainder via the extended alphabet:
   - `0..9, A..Z` → 0..31 (the standard alphabet)
   - `*` → 32, `~` → 33, `$` → 34, `=` → 35, `U` → 36
4. Append the resulting character to the encoded string.

Decode: strip the last character, recompute, compare. Mismatch → `Error::CheckMismatch`.

## 5. Testing Strategy

| Layer | Where | Run by |
|---|---|---|
| Unit (inline `#[cfg(test)]`) | each `src/*.rs` | `cargo test --lib` |
| Integration (round-trip + spec vectors) | `tests/integration.rs` | `cargo test --tests` |
| Doc-tests | every public function | bundled with `cargo test` |
| Example-as-test | `examples/basic.rs` | `cargo build --examples` |

**Specific test obligations:**

| File | Test |
|---|---|
| `error.rs` | Display rendering of each variant |
| `bytes.rs` | Crockford spec test vectors; empty round-trip; lenient decode (I/L/O); case-insensitivity; hyphen stripping; invalid-char position |
| `u64.rs` | `encode_u64(0)` length+value; `encode_u64(u64::MAX)` length+value; round-trip across `0..256` and 1000 random values; overflow on 14+ chars; empty → 0 |
| `check.rs` | Mod-37,5 vector from Crockford spec page; corrupting last char → `CheckMismatch`; corrupting body → `CheckMismatch` (recomputed check differs); empty → `Empty` |
| `tests/integration.rs` | 1KB random Vec<u8> round-trip; combined u64+bytes ULID-style round-trip; real Crockford spec test strings |

**Coverage target:** ≥90% per file (matches workspace bar after gap-fill).

## 6. Cross-Crate Integration

- **No `altair-otel` integration.** Pure encoding has no observability value. Users who care can wrap their own usage in `#[instrument]`.
- **`prelude` module** contains: the 6 functions, `Error`, `Result`. Nothing else.
- **No re-exports of `base32`.** That crate is purely an implementation detail; not in the public surface.

## 7. Out of Scope (v0.1.0)

Explicitly not included; revisit only if demand surfaces:

- `encode_u64_with_check` — convert u64 → 8 bytes → `encode_with_check`
- Streaming encode/decode — current target is small inputs (IDs, short tokens, license keys). Memory is not a problem.
- Hyphen insertion on encode — encode produces clean output; users can format as needed. Decode already strips them.
- Custom alphabets — this is the Crockford crate, not a Base32 toolkit. Users wanting RFC 4648 should use the `base32` crate directly.
- Padding control — Crockford spec doesn't use padding.
- `no_std` support — workspace targets `std`. Can be revisited later if asked.

## 8. Risks & Open Questions

| Item | Risk | Mitigation |
|---|---|---|
| `base32` crate API change (we depend on `Alphabet::Crockford` enum variant) | Low — crate is stable at v0.5.x with 22M downloads | Pin via `[workspace.dependencies]`; absorb breaking changes as our own minor bump |
| Re-implementing u64 path could diverge from spec | Low — algorithm is trivially testable | RFC vectors + property-style round-trip tests for 1000 random values |
| Check-digit alphabet uses `*`, `~`, `$`, `=` which aren't URL-safe | Documented constraint of Crockford spec — not a bug | Note in README that `encode_with_check` output may need URL-encoding for use in URLs |
| `position` reporting after hyphen-stripping could surprise users | Medium — could disagree with user's mental model of "char N of my input" | Document explicitly: position is in the *original* input string |

## 9. Versioning

- Starts at `0.1.0` per workspace convention.
- API is small and stable — most likely path to `1.0` once the workspace as a whole stabilizes.
- Re-export of `base32` types is **not** part of the public API, so `base32` upgrades won't force a breaking change here.

## 10. Next Steps

1. **User reviews this spec** (current step)
2. On approval: `writing-plans` skill produces an implementation plan
3. Implementation plan drives: crate scaffolding → per-module TDD → testing → CI → first crates.io publish as `0.1.0`
