//! Each `Error` variant carries actionable detail. Pattern-match on it to
//! decide how to respond.
//!
//! Run with: `cargo run --example error_handling -p altair-base32`

use altair_base32::prelude::*;

fn classify(input: &str) -> String {
    match decode(input) {
        Ok(bytes) => format!("OK ({} bytes)", bytes.len()),
        Err(Error::InvalidChar { ch, position }) => {
            format!("invalid char {ch:?} at byte position {position}")
        }
        Err(Error::Empty) => "input was empty".into(),
        Err(other) => format!("other: {other}"),
    }
}

fn main() {
    let cases = [
        ("CSQPYRK1E8", "valid: encoded 'foobar'"),
        ("csqpyrk1e8", "valid: lowercase ok"),
        ("CSQ-PYR-K1E-8", "valid: hyphens stripped"),
        ("CSQ?PYR", "invalid: ? at position 3"),
        ("U", "invalid: U is excluded from the Crockford alphabet"),
        ("", "valid: empty input → empty bytes"),
    ];

    for (input, desc) in cases {
        println!("{:30}  {}", format!("decode({input:?})"), desc);
        println!("  → {}", classify(input));
        println!();
    }

    // u64 overflow:
    match decode_u64("ZZZZZZZZZZZZZZ") {
        Err(Error::Overflow) => {
            println!("decode_u64('ZZZZZZZZZZZZZZ') = Overflow (too big for u64)");
        }
        other => println!("(unexpected) {other:?}"),
    }

    // Check-digit mismatch:
    match decode_with_check("CR") {
        Err(Error::CheckMismatch { expected, found }) => {
            println!(
                "decode_with_check('CR') = CheckMismatch (expected '{expected}', got '{found}')"
            );
        }
        other => println!("(unexpected) {other:?}"),
    }
}
