//! Pure tar archiving (no compression). Useful when the contents are
//! already compressed (e.g. pre-compressed assets, gzipped logs, .jpg).
//!
//! Run with: `cargo run --example tar_directory -p altair-compress`

use altair_compress::prelude::*;
use std::fs;
use std::io::Write;

fn main() -> anyhow::Result<()> {
    let work = tempfile::TempDir::new()?;
    let src = work.path().join("project");
    fs::create_dir_all(src.join("docs"))?;
    fs::File::create(src.join("README.md"))?.write_all(b"# demo\n")?;
    fs::File::create(src.join("Cargo.toml"))?.write_all(b"[package]\nname=\"demo\"\n")?;
    fs::File::create(src.join("docs").join("guide.md"))?.write_all(b"# guide\n")?;

    let archive = work.path().join("project.tar");
    tar_dir(&src, &archive)?;

    let restored = work.path().join("restored");
    untar(&archive, &restored)?;

    println!("archive size: {} bytes", fs::metadata(&archive)?.len());
    println!("contents:");
    visit(&restored, 0)?;
    Ok(())
}

fn visit(dir: &std::path::Path, depth: usize) -> anyhow::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let pad = "  ".repeat(depth);
        let name = entry.file_name();
        if entry.file_type()?.is_dir() {
            println!("{pad}{}/", name.to_string_lossy());
            visit(&entry.path(), depth + 1)?;
        } else {
            let size = entry.metadata()?.len();
            println!("{pad}{} ({size} bytes)", name.to_string_lossy());
        }
    }
    Ok(())
}
