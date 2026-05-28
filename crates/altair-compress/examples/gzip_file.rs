//! Single-file gzip compression and decompression.
//!
//! Run with: `cargo run --example gzip_file -p altair-compress`

#![allow(clippy::cast_precision_loss)] // size ratios — precision loss is acceptable here

use altair_compress::prelude::*;
use std::fs;
use std::io::Write;

fn main() -> anyhow::Result<()> {
    let work = tempfile::TempDir::new()?;
    let plain = work.path().join("payload.txt");
    let gz = work.path().join("payload.txt.gz");
    let back = work.path().join("payload_restored.txt");

    // 8 KB of repetitive text — compresses well.
    let pattern = b"the quick brown fox jumps over the lazy dog\n";
    let mut input = Vec::new();
    while input.len() < 8 * 1024 {
        input.extend_from_slice(pattern);
    }
    fs::File::create(&plain)?.write_all(&input)?;

    compress_file(&plain, &gz)?;
    decompress_file(&gz, &back)?;

    let plain_size = fs::metadata(&plain)?.len();
    let gz_size = fs::metadata(&gz)?.len();
    let back_size = fs::metadata(&back)?.len();

    println!("plain:       {plain_size} bytes");
    println!(
        "gzipped:     {gz_size} bytes  (ratio: {:.1}x)",
        plain_size as f64 / gz_size as f64
    );
    println!(
        "decompressed: {back_size} bytes  (matches plain: {})",
        back_size == plain_size
    );

    assert_eq!(fs::read(&plain)?, fs::read(&back)?);
    Ok(())
}
