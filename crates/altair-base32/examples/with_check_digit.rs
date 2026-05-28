//! Mod-37 check digit catches single-character corruption — useful for
//! serial numbers, license keys, anything users will type.
//!
//! Run with: `cargo run --example with_check_digit -p altair-base32`

use altair_base32::prelude::*;

fn main() {
    let serial: &[u8] = b"ORDER-2026-CHECKOUT";
    let encoded = encode_with_check(serial);
    println!("payload:    {serial:?}");
    println!("encoded:    {encoded}   (last char is the Mod-37 check digit)");
    println!();

    // Round-trip works.
    match decode_with_check(&encoded) {
        Ok(b) => assert_eq!(b, serial),
        Err(e) => panic!("round-trip failed: {e}"),
    }
    println!("round-trip via decode_with_check: OK");

    // Corrupt one character in the body and re-decode.
    let mut corrupted = encoded.clone();
    let first = corrupted.as_bytes()[0];
    let new_first = if first == b'C' { b'D' } else { b'C' };
    corrupted.replace_range(0..1, std::str::from_utf8(&[new_first]).unwrap());
    println!("corrupted:  {corrupted}   (first char flipped)");

    match decode_with_check(&corrupted) {
        Err(Error::CheckMismatch { expected, found }) => {
            println!("decode rejected the corruption: expected check '{expected}', got '{found}'");
        }
        Ok(_) => println!("(unexpected) decoded fine"),
        Err(other) => println!("other: {other}"),
    }
}
