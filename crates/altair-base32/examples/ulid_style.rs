//! ULID-style IDs: 13 chars of millisecond timestamp + 16 chars of random
//! bytes (10 bytes encoded as Crockford). Sortable by time, globally unique
//! within practical limits.
//!
//! Run with: `cargo run --example ulid_style -p altair-base32`

use altair_base32::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn ulid_now(random_bytes: [u8; 10]) -> String {
    // `as_millis()` returns u128 since 1970; in practice the value comfortably
    // fits in u64 for the next ~580 million years.
    let ts_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX);
    format!("{}{}", encode_u64(ts_ms), encode(&random_bytes))
}

fn split_ulid(ulid: &str) -> anyhow::Result<(u64, Vec<u8>)> {
    if ulid.len() != 13 + 16 {
        anyhow::bail!("expected 29-char ULID, got {}", ulid.len());
    }
    let ts = decode_u64(&ulid[..13])?;
    let rand = decode(&ulid[13..])?;
    Ok((ts, rand))
}

fn main() -> anyhow::Result<()> {
    // In a real app, fill from a CSPRNG. We use a deterministic-looking
    // pattern here so the output is reproducible.
    let r1: [u8; 10] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x12, 0x34];
    let r2: [u8; 10] = [0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde];

    let id1 = ulid_now(r1);
    std::thread::sleep(std::time::Duration::from_millis(2));
    let id2 = ulid_now(r2);

    println!("id1: {id1}");
    println!("id2: {id2}");
    println!();
    println!("id1 < id2 lexicographically? {}", id1 < id2);
    println!("(id2 was generated later, so its timestamp prefix is larger)");
    println!();

    for id in [&id1, &id2] {
        let (ts, rand) = split_ulid(id)?;
        println!("split {id}");
        println!("  ts:   {ts} ms since epoch");
        println!("  rand: {rand:?}");
    }

    Ok(())
}
