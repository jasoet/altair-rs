//! Crockford's lenient decoding tolerates the typos humans make: I/L look
//! like 1, O looks like 0, case doesn't matter, and hyphens are stripped.
//!
//! Run with: `cargo run --example lenient_decoding -p altair-base32`

use altair_base32::prelude::*;

fn main() -> anyhow::Result<()> {
    let canonical_payload = b"foobar";
    let canonical = encode(canonical_payload);
    println!("canonical:  {canonical}");
    println!();

    // Four mistyped variants that all decode to the same bytes.
    let variants = [
        ("lowercase", canonical.to_ascii_lowercase()),
        (
            "L instead of 1, O instead of 0",
            canonical.replace('1', "L").replace('0', "O"),
        ),
        (
            "with hyphens for readability",
            canonical
                .as_bytes()
                .chunks(3)
                .map(|c| std::str::from_utf8(c).unwrap())
                .collect::<Vec<_>>()
                .join("-"),
        ),
        (
            "mixed: lowercase + hyphens + L/O",
            canonical
                .to_ascii_lowercase()
                .replace('1', "l")
                .replace('0', "o")
                .as_bytes()
                .chunks(4)
                .map(|c| std::str::from_utf8(c).unwrap())
                .collect::<Vec<_>>()
                .join("-"),
        ),
    ];

    for (label, input) in &variants {
        let decoded = decode(input)?;
        println!("  {label:36}  '{input}'");
        assert_eq!(decoded, canonical_payload);
    }
    println!();
    println!("all variants decoded to the same bytes: {canonical_payload:?}");

    Ok(())
}
