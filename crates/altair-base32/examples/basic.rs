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
