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
