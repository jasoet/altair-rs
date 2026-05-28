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
