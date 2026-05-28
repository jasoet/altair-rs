//! `untar`, `unzip`, and `untar_gz` reject archive entries that would write
//! outside the destination directory — the classic "zip-slip" vulnerability.
//!
//! This example crafts a malicious tar archive by hand and verifies that
//! extraction rejects it without touching the filesystem.
//!
//! Run with: `cargo run --example zip_slip_protection -p altair-compress`

use altair_compress::prelude::*;
use std::fs::File;
use std::io::{BufWriter, Write};

fn write_malicious_tar(path: &std::path::Path) -> anyhow::Result<()> {
    // Minimal valid ustar header for `../escape.txt` (would write outside dest).
    let mut header = [0u8; 512];
    let name = b"../escape.txt";
    header[..name.len()].copy_from_slice(name);
    header[100..108].copy_from_slice(b"0000644\0");
    header[108..116].copy_from_slice(b"0000000\0");
    header[116..124].copy_from_slice(b"0000000\0");
    header[124..136].copy_from_slice(b"00000000005\0"); // size = 5
    header[136..148].copy_from_slice(b"00000000000\0"); // mtime
    header[148..156].fill(b' '); // checksum placeholder
    header[156] = b'0'; // regular file
    header[257..263].copy_from_slice(b"ustar\0");

    let checksum: u32 = header.iter().map(|&b| u32::from(b)).sum();
    let cstr = format!("{checksum:06o}\0 ");
    header[148..156].copy_from_slice(cstr.as_bytes());

    let mut writer = BufWriter::new(File::create(path)?);
    writer.write_all(&header)?;
    writer.write_all(b"oops!")?;
    // Pad to 512 then write the archive terminator (two zero blocks).
    let pad = vec![0u8; 512 - 5];
    writer.write_all(&pad)?;
    writer.write_all(&[0u8; 1024])?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let work = tempfile::TempDir::new()?;
    let archive = work.path().join("malicious.tar");
    write_malicious_tar(&archive)?;

    let dest = work.path().join("safe");
    println!(
        "attempting to extract '{}' into '{}'",
        archive.display(),
        dest.display()
    );

    match untar(&archive, &dest) {
        Ok(()) => println!("(unexpected) extracted without error"),
        Err(Error::UnsafePath { path }) => {
            println!(
                "safely rejected: entry path '{}' would escape destination",
                path.display()
            );
            // Verify the destination is empty / nothing got written.
            let count = std::fs::read_dir(&dest)?.count();
            println!(
                "destination still contains {count} entries (none from the malicious archive)"
            );
        }
        Err(other) => println!("other error: {other}"),
    }
    Ok(())
}
