//! End-to-end behavior tests for altair-compress.

use altair_compress::{Error, prelude::*};
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

#[test]
fn gzip_decompress_respects_byte_limit() {
    // Compress 2 KiB, then try to decompress with a 1 KiB cap. Limit fires.
    let work = TempDir::new().unwrap();
    let input = work.path().join("payload.bin");
    let compressed = work.path().join("payload.bin.gz");
    let output = work.path().join("payload_restored.bin");

    let data = vec![b'A'; 2048];
    fs::File::create(&input).unwrap().write_all(&data).unwrap();
    compress_file(&input, &compressed).unwrap();

    let res = altair_compress::decompress_file_with_limit(&compressed, &output, 1024);
    assert!(
        matches!(
            res,
            Err(Error::DecompressionLimit {
                limit: 1024,
                kind: "gzip-stream"
            })
        ),
        "expected DecompressionLimit, got {res:?}",
    );
}

#[test]
fn gzip_decompress_within_limit_succeeds() {
    let work = TempDir::new().unwrap();
    let input = work.path().join("payload.bin");
    let compressed = work.path().join("payload.bin.gz");
    let output = work.path().join("payload_restored.bin");

    let data = vec![b'B'; 512];
    fs::File::create(&input).unwrap().write_all(&data).unwrap();
    compress_file(&input, &compressed).unwrap();

    altair_compress::decompress_file_with_limit(&compressed, &output, 1024).unwrap();
    assert_eq!(fs::read(&output).unwrap(), data);
}

#[test]
fn unzip_respects_byte_limit_across_entries() {
    // Build a zip with two ~1 KiB entries, then extract with a 1.5 KiB
    // cap — the second entry must trip the limit.
    let work = TempDir::new().unwrap();
    let src = work.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::File::create(src.join("a.bin"))
        .unwrap()
        .write_all(&vec![b'a'; 1024])
        .unwrap();
    fs::File::create(src.join("b.bin"))
        .unwrap()
        .write_all(&vec![b'b'; 1024])
        .unwrap();

    let archive = work.path().join("two.zip");
    zip_dir(&src, &archive).unwrap();

    let dest = work.path().join("out");
    let res = altair_compress::unzip_with_limit(&archive, &dest, 1500);
    assert!(
        matches!(
            res,
            Err(Error::DecompressionLimit {
                limit: 1500,
                kind: "zip-entry"
            })
        ),
        "expected DecompressionLimit, got {res:?}",
    );
}

#[test]
fn untar_gz_respects_byte_limit() {
    let src = make_fixture_dir();
    let work = TempDir::new().unwrap();
    let archive = work.path().join("project.tar.gz");
    let restored = work.path().join("restored");

    tar_gz_dir(src.path(), &archive).unwrap();

    // The fixture totals ~40 bytes across three files. Cap at 16 — must
    // trip on one of the entries.
    let res = altair_compress::untar_gz_with_limit(&archive, &restored, 16);
    assert!(
        matches!(
            res,
            Err(Error::DecompressionLimit {
                limit: 16,
                kind: "tar-entry"
            })
        ),
        "expected DecompressionLimit, got {res:?}",
    );
}
