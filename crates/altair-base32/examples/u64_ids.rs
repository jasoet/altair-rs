//! `encode_u64` produces 13-character zero-padded Crockford strings —
//! sortable in the same order as the underlying integers. Useful for
//! ULID/CUID-style ID generation.
//!
//! Run with: `cargo run --example u64_ids -p altair-base32`

use altair_base32::prelude::*;

fn main() -> anyhow::Result<()> {
    // Sortability check: encode a sequence and verify the resulting strings
    // sort the same way as the integers.
    let ids: Vec<u64> = vec![1, 31, 32, 1023, 1024, 1_000_000, u64::MAX / 2, u64::MAX];

    println!("integer-sorted        encoded                 round-trip");
    println!("---------------       -------------           ----------");
    for n in &ids {
        let s = encode_u64(*n);
        let back = decode_u64(&s)?;
        println!("{n:<20}  {s}           {back}");
    }
    println!();

    // Lexicographically sorting the encoded strings preserves the integer
    // ordering — the property ULID relies on for time-ordered IDs.
    let mut encoded: Vec<String> = ids.iter().map(|n| encode_u64(*n)).collect();
    encoded.sort();
    let resorted: Vec<u64> = encoded
        .iter()
        .map(|s| decode_u64(s))
        .collect::<Result<Vec<_>>>()?;
    println!("after lexicographic sort of encoded strings:");
    for n in &resorted {
        println!("  {n}");
    }

    Ok(())
}
