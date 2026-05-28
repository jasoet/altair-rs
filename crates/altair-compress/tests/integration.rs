//! End-to-end behavior tests for altair-compress.

use altair_compress::prelude::*;
use pretty_assertions::assert_eq;
use std::fs;
use std::io::Write;
use tempfile::TempDir;

fn make_fixture_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("src").join("nested")).unwrap();
    fs::File::create(root.join("Cargo.toml"))
        .unwrap()
        .write_all(b"[package]\nname = \"demo\"\n")
        .unwrap();
    fs::File::create(root.join("src").join("lib.rs"))
        .unwrap()
        .write_all(b"// lib\n")
        .unwrap();
    fs::File::create(root.join("src").join("nested").join("inner.rs"))
        .unwrap()
        .write_all(b"// inner\n")
        .unwrap();
    dir
}

#[test]
fn tar_gz_round_trip_preserves_contents() {
    let src = make_fixture_dir();
    let work = TempDir::new().unwrap();
    let archive = work.path().join("project.tar.gz");
    let restored = work.path().join("restored");

    tar_gz_dir(src.path(), &archive).unwrap();
    untar_gz(&archive, &restored).unwrap();

    assert_eq!(
        fs::read(restored.join("Cargo.toml")).unwrap(),
        fs::read(src.path().join("Cargo.toml")).unwrap(),
    );
    assert_eq!(
        fs::read(restored.join("src").join("lib.rs")).unwrap(),
        fs::read(src.path().join("src").join("lib.rs")).unwrap(),
    );
    assert_eq!(
        fs::read(restored.join("src").join("nested").join("inner.rs")).unwrap(),
        fs::read(src.path().join("src").join("nested").join("inner.rs")).unwrap(),
    );
}

#[test]
fn zip_round_trip_matches_tar() {
    let src = make_fixture_dir();
    let work = TempDir::new().unwrap();
    let zip_archive = work.path().join("project.zip");
    let tar_archive = work.path().join("project.tar");
    let zip_dest = work.path().join("from_zip");
    let tar_dest = work.path().join("from_tar");

    zip_dir(src.path(), &zip_archive).unwrap();
    tar_dir(src.path(), &tar_archive).unwrap();

    unzip(&zip_archive, &zip_dest).unwrap();
    untar(&tar_archive, &tar_dest).unwrap();

    assert_eq!(
        fs::read(zip_dest.join("Cargo.toml")).unwrap(),
        fs::read(tar_dest.join("Cargo.toml")).unwrap(),
    );
}

#[test]
fn compress_decompress_single_file_round_trip() {
    let work = TempDir::new().unwrap();
    let input = work.path().join("payload.bin");
    let compressed = work.path().join("payload.bin.gz");
    let output = work.path().join("payload_restored.bin");

    let data: Vec<u8> = b"the quick brown fox jumps over the lazy dog\n"
        .iter()
        .copied()
        .cycle()
        .take(1024)
        .collect();
    fs::File::create(&input).unwrap().write_all(&data).unwrap();

    compress_file(&input, &compressed).unwrap();
    decompress_file(&compressed, &output).unwrap();

    assert_eq!(fs::read(&output).unwrap(), data);
}
