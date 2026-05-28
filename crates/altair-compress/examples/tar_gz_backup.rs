//! Realistic backup workflow: tar.gz a directory tree, then extract it
//! elsewhere to verify the round-trip.
//!
//! Run with: `cargo run --example tar_gz_backup -p altair-compress`

use altair_compress::prelude::*;
use std::fs;
use std::io::Write;

fn main() -> anyhow::Result<()> {
    let work = tempfile::TempDir::new()?;

    // Build a small project tree.
    let project = work.path().join("my-app");
    fs::create_dir_all(project.join("src").join("nested"))?;
    fs::create_dir_all(project.join("docs"))?;

    fs::File::create(project.join("Cargo.toml"))?.write_all(b"[package]\nname=\"my-app\"\n")?;
    fs::File::create(project.join("README.md"))?.write_all(b"# my-app\n\nbackup demo\n")?;
    fs::File::create(project.join("src").join("main.rs"))?.write_all(b"fn main() {}\n")?;
    fs::File::create(project.join("src").join("nested").join("lib.rs"))?.write_all(b"// lib\n")?;
    fs::File::create(project.join("docs").join("CHANGELOG.md"))?.write_all(b"# changelog\n")?;

    // Tarball it.
    let backup = work.path().join("backups").join("my-app-2026-05-28.tar.gz");
    fs::create_dir_all(backup.parent().unwrap())?;
    tar_gz_dir(&project, &backup)?;
    println!("created backup: {}", backup.display());
    println!("size: {} bytes", fs::metadata(&backup)?.len());

    // Restore it somewhere else and verify.
    let restored = work.path().join("restored");
    untar_gz(&backup, &restored)?;
    println!();
    println!("restored to: {}", restored.display());

    // Sanity-check a few files.
    for relative in [
        "Cargo.toml",
        "README.md",
        "src/main.rs",
        "src/nested/lib.rs",
        "docs/CHANGELOG.md",
    ] {
        let original = fs::read(project.join(relative))?;
        let copy = fs::read(restored.join(relative))?;
        assert_eq!(original, copy);
        println!("  {relative}: ok");
    }

    Ok(())
}
