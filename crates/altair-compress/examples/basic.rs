//! Run with: `cargo run --example basic -p altair-compress`

use altair_compress::prelude::*;
use std::fs;
use std::io::Write;

fn main() -> anyhow::Result<()> {
    let work = tempfile::TempDir::new()?;
    let project = work.path().join("project");
    fs::create_dir_all(project.join("src"))?;
    fs::File::create(project.join("README.md"))?.write_all(b"# demo\n")?;
    fs::File::create(project.join("src").join("lib.rs"))?.write_all(b"// lib\n")?;

    // 1. tar a directory
    let tar_path = work.path().join("project.tar");
    tar_dir(&project, &tar_path)?;
    println!("tar archive: {} bytes", fs::metadata(&tar_path)?.len());

    // 2. zip a directory
    let zip_path = work.path().join("project.zip");
    zip_dir(&project, &zip_path)?;
    println!("zip archive: {} bytes", fs::metadata(&zip_path)?.len());

    // 3. tar.gz the same directory
    let tarball_path = work.path().join("project.tar.gz");
    tar_gz_dir(&project, &tarball_path)?;
    println!("tar.gz archive: {} bytes", fs::metadata(&tarball_path)?.len());

    // 4. Round-trip the tarball
    let restored = work.path().join("restored");
    untar_gz(&tarball_path, &restored)?;
    println!("restored README: {:?}", fs::read_to_string(restored.join("README.md"))?);

    Ok(())
}
