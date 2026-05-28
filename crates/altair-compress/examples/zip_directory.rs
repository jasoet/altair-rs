//! Zip archiving with DEFLATE compression. Each entry is compressed
//! independently; well suited for archives someone will open with a GUI tool.
//!
//! Run with: `cargo run --example zip_directory -p altair-compress`

#![allow(clippy::cast_precision_loss)] // size ratios

use altair_compress::prelude::*;
use std::fs;
use std::io::Write;

fn main() -> anyhow::Result<()> {
    let work = tempfile::TempDir::new()?;
    let src = work.path().join("project");
    fs::create_dir_all(src.join("src"))?;

    // Make the contents compressible so the zip is meaningfully smaller.
    let pattern = b"the quick brown fox jumps over the lazy dog\n";
    let mut data = Vec::new();
    while data.len() < 4096 {
        data.extend_from_slice(pattern);
    }
    fs::File::create(src.join("notes.txt"))?.write_all(&data)?;
    fs::File::create(src.join("src").join("lib.rs"))?.write_all(b"// lib\n")?;

    let archive = work.path().join("project.zip");
    zip_dir(&src, &archive)?;

    let plain_size: u64 = walk_size(&src)?;
    let zip_size = fs::metadata(&archive)?.len();

    println!("source total:  {plain_size} bytes");
    println!("zip total:     {zip_size} bytes");
    println!("ratio:         {:.2}x", plain_size as f64 / zip_size as f64);

    let restored = work.path().join("restored");
    unzip(&archive, &restored)?;
    assert_eq!(fs::read(restored.join("notes.txt"))?, data);
    println!("round-trip:    OK");
    Ok(())
}

fn walk_size(dir: &std::path::Path) -> anyhow::Result<u64> {
    let mut total = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            total += walk_size(&entry.path())?;
        } else {
            total += entry.metadata()?.len();
        }
    }
    Ok(total)
}
