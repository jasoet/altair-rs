# altair-compress Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build, test, and publish `altair-compress` — path-based recipes for gzip, tar, zip, and tar.gz with zip-slip protection — to crates.io at the current workspace version.

**Architecture:** Single crate under `crates/altair-compress/`. Seven source files: `lib.rs`, `error.rs`, `gzip.rs`, `tar.rs`, `zip.rs`, `tarball.rs`, `safe_path.rs`, `prelude.rs`. Each format module wraps exactly one library (`flate2`, `tar`, `zip`). `tarball.rs` composes `tar` + `flate2` for the dominant `.tar.gz` case. `safe_path.rs` provides the zip-slip mitigation helper used by every extract path.

**Tech Stack:**
- Rust 2024, MSRV 1.95 (inherit from workspace)
- `flate2 = "1"` — gzip stream encode/decode
- `tar = "0.4"` — TAR archive create/extract
- `zip = "0.8"` — ZIP archive create/extract (pinned to v8.x stable; v9 is pre-release)
- `thiserror = "2"` (workspace)
- `tempfile = "3"` (workspace) — for integration tests

---

## File Structure

```
crates/altair-compress/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs        # crate root: lints, mod declarations, re-exports
│   ├── error.rs      # Error enum + Result alias
│   ├── safe_path.rs  # pub(crate) resolve() — zip-slip mitigation
│   ├── gzip.rs       # compress_file / decompress_file
│   ├── tar.rs        # tar_dir / untar
│   ├── zip.rs        # zip_dir / unzip
│   ├── tarball.rs    # tar_gz_dir / untar_gz
│   └── prelude.rs    # one-import bundle
├── tests/
│   └── integration.rs
└── examples/
    └── basic.rs
```

Workspace edits:
- `Cargo.toml`: add `flate2 = "1"`, `tar = "0.4"`, `zip = "0.8"` to `[workspace.dependencies]`; add `crates/altair-compress` to `members`
- `docs/porting-tracker.md`: move `altair-compress` from Deferred → Done; add release notes bullet
- `README.md`: add `altair-compress` row to crate table

---

## Phase 1: Crate Scaffold

### Task 1.1: Add libraries to workspace dependencies

**Files:**
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Add the three deps**

In the root `Cargo.toml`'s `[workspace.dependencies]` block, add an "# Archiving / compression" section:

```toml
# Archiving / compression
flate2 = "1"
tar = "0.4"
zip = "0.8"
```

A reasonable placement is near the existing `base32 = "0.5"` `# Encoding` section.

- [ ] **Step 2: Verify workspace parses**

Run: `cargo metadata --format-version=1 > /dev/null`
Expected: exit 0.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add flate2, tar, zip to workspace dependencies"
```

### Task 1.2: Create crate skeleton

**Files:**
- Create: `crates/altair-compress/Cargo.toml`
- Create: `crates/altair-compress/src/lib.rs`
- Create: `crates/altair-compress/README.md` (stub)
- Modify: `Cargo.toml` (workspace `members`)

- [ ] **Step 1: Create directories**

Run: `mkdir -p crates/altair-compress/src crates/altair-compress/tests crates/altair-compress/examples`

- [ ] **Step 2: Write `crates/altair-compress/Cargo.toml`**

```toml
[package]
name = "altair-compress"
description = "Path-based recipes for gzip, tar, and zip with zip-slip protection"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
homepage.workspace = true
readme = "README.md"
keywords = ["compression", "gzip", "tar", "zip", "archive"]
categories = ["compression", "filesystem"]

[dependencies]
flate2 = { workspace = true }
tar = { workspace = true }
zip = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
pretty_assertions = { workspace = true }
anyhow = { workspace = true }
tempfile = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 3: Write `crates/altair-compress/src/lib.rs`** (declares modules and public re-exports)

```rust
//! Path-based recipes for gzip, tar, and zip — plus the dominant tar.gz combo.
//!
//! Wraps battle-tested libraries ([`flate2`], [`tar`], [`zip`]) with smart
//! defaults, typed errors, and zip-slip protection. The underlying libraries
//! are re-exported at the crate root for power users who need custom
//! compression levels, builder-style archive construction, or other features
//! beyond the recipes.
//!
//! # Example
//!
//! ```no_run
//! use altair_compress::{tar_gz_dir, untar_gz};
//!
//! # fn run() -> altair_compress::Result<()> {
//! tar_gz_dir("./my-project", "/tmp/backup.tar.gz")?;
//! untar_gz("/tmp/backup.tar.gz", "/tmp/restored")?;
//! # Ok(()) }
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod error;
mod gzip;
mod safe_path;
mod tar;
mod tarball;
mod zip;

pub mod prelude;

pub use error::{Error, Result};
pub use gzip::{compress_file, decompress_file};
pub use tar::{tar_dir, untar};
pub use tarball::{tar_gz_dir, untar_gz};
pub use zip::{unzip, zip_dir};

// Re-exports for one-dep ergonomics. Note `flate2`, `tar`, and `zip` are also
// the names of our own modules — those modules are `mod` (private) above; the
// `pub use` here exports the *external* crates under the same names.
pub use ::flate2;
pub use ::tar;
pub use ::zip;
```

- [ ] **Step 4: Write stub README**

```markdown
# altair-compress

Path-based recipes for gzip, tar, and zip with zip-slip protection.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

(Full README added in a later task.)
```

- [ ] **Step 5: Register in workspace `members`**

In root `Cargo.toml`, append `"crates/altair-compress"` to the `members = [ ... ]` list:

```toml
members = [
    "crates/altair-concurrent",
    "crates/altair-retry",
    "crates/altair-config",
    "crates/altair-otel",
    "crates/altair-base32",
    "crates/altair-compress",
]
```

- [ ] **Step 6: Verify the workspace parses**

Run: `cargo metadata --format-version=1 > /dev/null`
Expected: exit 0. (`cargo build -p altair-compress` will still fail with "file not found" for the missing modules — that's expected and resolved in subsequent tasks.)

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/altair-compress
git commit -m "feat(compress): scaffold altair-compress crate"
```

---

## Phase 2: Error type

### Task 2.1: Write `error.rs` with tests

**Files:**
- Create: `crates/altair-compress/src/error.rs`

- [ ] **Step 1: Write the file**

```rust
//! Crate-wide error type for `altair-compress`.

use thiserror::Error;

/// Errors returned by `altair-compress` recipes.
#[derive(Debug, Error)]
pub enum Error {
    /// I/O failure during read/write (open, seek, copy, etc.). Most underlying
    /// `flate2` and `tar` failures surface here via the `#[from]` conversion.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Compression-layer error from `flate2` or `tar` that isn't already an
    /// `io::Error` (rare; stringified for simplicity).
    #[error("compression: {0}")]
    Compression(String),

    /// Error from the `zip` library (CRC mismatch, unsupported method, etc.).
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// Refused to extract an archive entry whose path would write outside
    /// the destination directory (zip-slip / tar-slip mitigation).
    #[error("entry path escapes destination: {path:?}")]
    UnsafePath {
        /// The offending entry path as recorded in the archive.
        path: std::path::PathBuf,
    },

    /// Source path doesn't exist or isn't of the expected kind
    /// (e.g. expected a directory, got a file).
    #[error("invalid source: {path:?}: {reason}")]
    InvalidSource {
        /// The path that was rejected.
        path: std::path::PathBuf,
        /// Why it was rejected.
        reason: String,
    },
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn io_error_renders() {
        let e: Error = std::io::Error::other("disk full").into();
        assert!(e.to_string().contains("disk full"));
    }

    #[test]
    fn compression_renders() {
        let e = Error::Compression("bad header".into());
        assert_eq!(e.to_string(), "compression: bad header");
    }

    #[test]
    fn unsafe_path_renders_path() {
        let e = Error::UnsafePath {
            path: PathBuf::from("../etc/passwd"),
        };
        assert!(e.to_string().contains("../etc/passwd"));
    }

    #[test]
    fn invalid_source_renders_reason() {
        let e = Error::InvalidSource {
            path: PathBuf::from("/tmp/oops"),
            reason: "not a directory".into(),
        };
        let s = e.to_string();
        assert!(s.contains("/tmp/oops"));
        assert!(s.contains("not a directory"));
    }
}
```

- [ ] **Step 2: Run tests**

Note: the full crate won't link until later tasks fill in `safe_path.rs`, `gzip.rs`, etc. Use the same trick as in `altair-base32`: temporarily comment out the unfilled `mod` lines and `pub use` lines in `crates/altair-compress/src/lib.rs`, run `cargo test -p altair-compress --lib error`, then restore lib.rs and commit ONLY `error.rs`.

Run: `cargo test -p altair-compress --lib error`
Expected: 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-compress/src/error.rs
git commit -m "feat(compress): add Error type and Result alias"
```

Only `error.rs` should be in the commit. Do NOT commit temporary edits to `lib.rs`.

---

## Phase 3: Zip-slip safe-path helper

### Task 3.1: Write `safe_path.rs` with tests

**Files:**
- Create: `crates/altair-compress/src/safe_path.rs`

- [ ] **Step 1: Write the file**

```rust
//! Zip-slip / tar-slip mitigation helper.
//!
//! [`resolve`] joins an archive entry path under a destination root and
//! verifies the canonicalized result stays inside. Any entry containing
//! `..` components, absolute paths, or symlinks pointing outside the
//! destination is rejected with [`crate::Error::UnsafePath`].

use crate::error::{Error, Result};
use std::path::{Component, Path, PathBuf};

/// Resolve an archive entry's destination path, rejecting anything that
/// would escape `dest_root`.
///
/// Returns the joined `dest_root / entry_path` after validating that the
/// path doesn't contain `..` components or absolute components.
///
/// Note: this is a path-component check, not a canonicalize-based check.
/// We deliberately avoid `Path::canonicalize` here because the destination
/// path doesn't exist yet (we're computing where to write *to*). Rejecting
/// `..` and absolute components is sufficient for the threat model
/// (malicious archive entries), and works uniformly across platforms.
pub(crate) fn resolve(dest_root: &Path, entry_path: &Path) -> Result<PathBuf> {
    for component in entry_path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                return Err(Error::UnsafePath {
                    path: entry_path.to_path_buf(),
                });
            }
            Component::ParentDir => {
                return Err(Error::UnsafePath {
                    path: entry_path.to_path_buf(),
                });
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(dest_root.join(entry_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn accepts_simple_entry() {
        let out = resolve(Path::new("/dst"), Path::new("file.txt")).unwrap();
        assert_eq!(out, PathBuf::from("/dst/file.txt"));
    }

    #[test]
    fn accepts_nested_entry() {
        let out = resolve(Path::new("/dst"), Path::new("sub/a.txt")).unwrap();
        assert_eq!(out, PathBuf::from("/dst/sub/a.txt"));
    }

    #[test]
    fn rejects_parent_dir() {
        match resolve(Path::new("/dst"), Path::new("../etc/passwd")) {
            Err(Error::UnsafePath { path }) => {
                assert_eq!(path, PathBuf::from("../etc/passwd"));
            }
            other => panic!("expected UnsafePath, got {other:?}"),
        }
    }

    #[test]
    fn rejects_parent_dir_in_middle() {
        match resolve(Path::new("/dst"), Path::new("safe/../../escape")) {
            Err(Error::UnsafePath { .. }) => {}
            other => panic!("expected UnsafePath, got {other:?}"),
        }
    }

    #[test]
    fn rejects_absolute_path_unix() {
        match resolve(Path::new("/dst"), Path::new("/etc/passwd")) {
            Err(Error::UnsafePath { .. }) => {}
            other => panic!("expected UnsafePath, got {other:?}"),
        }
    }

    #[test]
    fn accepts_current_dir_components() {
        let out = resolve(Path::new("/dst"), Path::new("./file.txt")).unwrap();
        assert_eq!(out, PathBuf::from("/dst/./file.txt"));
    }
}
```

- [ ] **Step 2: Run tests**

Use the same temporary-comment-out trick on `lib.rs` if needed. Run:

`cargo test -p altair-compress --lib safe_path`
Expected: 6 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-compress/src/safe_path.rs
git commit -m "feat(compress): add safe_path::resolve for zip-slip mitigation"
```

---

## Phase 4: gzip recipe

### Task 4.1: Write `gzip.rs` with tests

**Files:**
- Create: `crates/altair-compress/src/gzip.rs`

- [ ] **Step 1: Write the file**

```rust
//! Single-file gzip compression via `flate2`.

use crate::error::Result;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;

/// Compress `input` to `output` using gzip at the default compression level.
///
/// Both paths are file paths. `output`'s parent directory must exist.
///
/// ```no_run
/// altair_compress::compress_file("data.bin", "data.bin.gz").unwrap();
/// ```
pub fn compress_file(input: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<()> {
    let input_file = File::open(input.as_ref())?;
    let mut reader = BufReader::new(input_file);
    let output_file = File::create(output.as_ref())?;
    let writer = BufWriter::new(output_file);
    let mut encoder = GzEncoder::new(writer, Compression::default());
    io::copy(&mut reader, &mut encoder)?;
    encoder.finish()?;
    Ok(())
}

/// Decompress a gzip file to `output`.
///
/// ```no_run
/// altair_compress::decompress_file("data.bin.gz", "data.bin").unwrap();
/// ```
pub fn decompress_file(input: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<()> {
    let input_file = File::open(input.as_ref())?;
    let mut decoder = GzDecoder::new(BufReader::new(input_file));
    let output_file = File::create(output.as_ref())?;
    let mut writer = BufWriter::new(output_file);
    io::copy(&mut decoder, &mut writer)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use tempfile::TempDir;

    #[test]
    fn round_trip_1kb_file() {
        let dir = TempDir::new().unwrap();
        let input = dir.path().join("in.bin");
        let compressed = dir.path().join("in.bin.gz");
        let output = dir.path().join("out.bin");

        let payload: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
        File::create(&input).unwrap().write_all(&payload).unwrap();

        compress_file(&input, &compressed).unwrap();
        decompress_file(&compressed, &output).unwrap();

        let mut roundtripped = Vec::new();
        File::open(&output).unwrap().read_to_end(&mut roundtripped).unwrap();
        assert_eq!(roundtripped, payload);
    }

    #[test]
    fn round_trip_empty_file() {
        let dir = TempDir::new().unwrap();
        let input = dir.path().join("in.bin");
        let compressed = dir.path().join("in.bin.gz");
        let output = dir.path().join("out.bin");

        File::create(&input).unwrap();
        compress_file(&input, &compressed).unwrap();
        decompress_file(&compressed, &output).unwrap();

        let metadata = std::fs::metadata(&output).unwrap();
        assert_eq!(metadata.len(), 0);
    }

    #[test]
    fn missing_source_yields_io_error() {
        let dir = TempDir::new().unwrap();
        let result = compress_file(dir.path().join("nonexistent"), dir.path().join("out.gz"));
        match result {
            Err(crate::error::Error::Io(_)) => {}
            other => panic!("expected Io, got {other:?}"),
        }
    }

    #[test]
    fn decompressing_garbage_yields_io_error() {
        let dir = TempDir::new().unwrap();
        let bogus = dir.path().join("not_a_gzip.bin");
        File::create(&bogus)
            .unwrap()
            .write_all(b"this is not a valid gzip stream")
            .unwrap();
        let output = dir.path().join("out.bin");
        let result = decompress_file(&bogus, &output);
        // GzDecoder surfaces the malformed-header error as io::Error
        match result {
            Err(crate::error::Error::Io(_)) => {}
            other => panic!("expected Io, got {other:?}"),
        }
    }
}
```

> Note on the `(i % 256) as u8` cast: clippy will flag `cast_possible_truncation` in pedantic mode. Wrap the test with `#[allow(clippy::cast_possible_truncation)]` if needed; the truncation is intentional (modulo to a byte).

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-compress --lib gzip`
Expected: 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-compress/src/gzip.rs
git commit -m "feat(compress): gzip compress_file / decompress_file recipes"
```

---

## Phase 5: tar recipe

### Task 5.1: Write `tar.rs` with tests

**Files:**
- Create: `crates/altair-compress/src/tar.rs`

- [ ] **Step 1: Write the file**

```rust
//! Directory archiving via `tar` (no compression).

use crate::error::{Error, Result};
use crate::safe_path;
use std::fs::{File, create_dir_all};
use std::io::{BufReader, BufWriter};
use std::path::Path;

/// Archive `source_dir`'s contents recursively into a tar file at `output`.
///
/// Entry paths in the archive are stored **relative to `source_dir`** (the
/// source root itself doesn't appear as a top-level component). So
/// `tar_dir("/a/b", "/tmp/out.tar")` records `/a/b/c.txt` as `c.txt`, not
/// as `b/c.txt`.
///
/// Returns [`Error::InvalidSource`] if `source_dir` isn't a directory.
///
/// ```no_run
/// altair_compress::tar_dir("./my-project", "/tmp/proj.tar").unwrap();
/// ```
pub fn tar_dir(source_dir: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<()> {
    let source = source_dir.as_ref();
    if !source.is_dir() {
        return Err(Error::InvalidSource {
            path: source.to_path_buf(),
            reason: "not a directory".into(),
        });
    }
    let output_file = File::create(output.as_ref())?;
    let writer = BufWriter::new(output_file);
    let mut builder = ::tar::Builder::new(writer);
    // append_dir_all stores entries relative to the prefix ("" means root of archive)
    builder.append_dir_all("", source)?;
    builder.finish()?;
    Ok(())
}

/// Extract a tar archive to `dest_dir`, creating it if it doesn't exist.
///
/// Rejects entries whose path would write outside `dest_dir`
/// ([`Error::UnsafePath`]).
///
/// ```no_run
/// altair_compress::untar("/tmp/proj.tar", "/tmp/restored").unwrap();
/// ```
pub fn untar(archive: impl AsRef<Path>, dest_dir: impl AsRef<Path>) -> Result<()> {
    let dest = dest_dir.as_ref();
    create_dir_all(dest)?;
    let archive_file = File::open(archive.as_ref())?;
    let mut archive = ::tar::Archive::new(BufReader::new(archive_file));

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?.into_owned();
        let safe_dest = safe_path::resolve(dest, &entry_path)?;
        entry.unpack(&safe_dest)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_fixture_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::File::create(root.join("a.txt"))
            .unwrap()
            .write_all(b"alpha")
            .unwrap();
        fs::File::create(root.join("sub").join("b.txt"))
            .unwrap()
            .write_all(b"beta")
            .unwrap();
        dir
    }

    #[test]
    fn round_trip_directory_tree() {
        let src = make_fixture_dir();
        let work = TempDir::new().unwrap();
        let archive = work.path().join("out.tar");
        let restored = work.path().join("restored");

        tar_dir(src.path(), &archive).unwrap();
        untar(&archive, &restored).unwrap();

        let a_content = fs::read_to_string(restored.join("a.txt")).unwrap();
        let b_content = fs::read_to_string(restored.join("sub").join("b.txt")).unwrap();
        assert_eq!(a_content, "alpha");
        assert_eq!(b_content, "beta");
    }

    #[test]
    fn non_directory_source_rejected() {
        let work = TempDir::new().unwrap();
        let file = work.path().join("not_a_dir.txt");
        fs::File::create(&file).unwrap();
        let archive = work.path().join("out.tar");
        let result = tar_dir(&file, &archive);
        match result {
            Err(Error::InvalidSource { reason, .. }) => {
                assert!(reason.contains("not a directory"));
            }
            other => panic!("expected InvalidSource, got {other:?}"),
        }
    }

    #[test]
    fn untar_rejects_parent_dir_entry() {
        // Hand-craft a tar archive containing an entry "../escape.txt"
        let work = TempDir::new().unwrap();
        let archive = work.path().join("malicious.tar");
        {
            let writer = BufWriter::new(File::create(&archive).unwrap());
            let mut builder = ::tar::Builder::new(writer);
            let mut header = ::tar::Header::new_gnu();
            header.set_path("../escape.txt").unwrap();
            header.set_size(5);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append(&header, b"oops!" as &[u8]).unwrap();
            builder.finish().unwrap();
        }

        let restored = work.path().join("restored");
        let result = untar(&archive, &restored);
        match result {
            Err(Error::UnsafePath { path }) => {
                assert_eq!(path.to_str(), Some("../escape.txt"));
            }
            other => panic!("expected UnsafePath, got {other:?}"),
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-compress --lib tar`
Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-compress/src/tar.rs
git commit -m "feat(compress): tar_dir / untar with zip-slip protection"
```

---

## Phase 6: zip recipe

### Task 6.1: Write `zip.rs` with tests

**Files:**
- Create: `crates/altair-compress/src/zip.rs`

- [ ] **Step 1: Write the file**

```rust
//! Directory archiving via `zip` (DEFLATE compression).

use crate::error::{Error, Result};
use crate::safe_path;
use std::fs::{File, create_dir_all};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;

/// Archive `source_dir`'s contents recursively into a zip file at `output`,
/// using DEFLATE compression for each entry.
///
/// Entry paths are stored relative to `source_dir`.
///
/// Returns [`Error::InvalidSource`] if `source_dir` isn't a directory.
///
/// ```no_run
/// altair_compress::zip_dir("./my-project", "/tmp/proj.zip").unwrap();
/// ```
pub fn zip_dir(source_dir: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<()> {
    let source = source_dir.as_ref();
    if !source.is_dir() {
        return Err(Error::InvalidSource {
            path: source.to_path_buf(),
            reason: "not a directory".into(),
        });
    }
    let output_file = File::create(output.as_ref())?;
    let writer = BufWriter::new(output_file);
    let mut zip_writer = ::zip::ZipWriter::new(writer);
    let options = SimpleFileOptions::default()
        .compression_method(::zip::CompressionMethod::Deflated);

    walk_and_add(&mut zip_writer, source, source, options)?;
    zip_writer.finish()?;
    Ok(())
}

fn walk_and_add<W: Write + std::io::Seek>(
    writer: &mut ::zip::ZipWriter<W>,
    source_root: &Path,
    current: &Path,
    options: SimpleFileOptions,
) -> Result<()> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path
            .strip_prefix(source_root)
            .map_err(|e| Error::Compression(format!("strip_prefix: {e}")))?;
        let relative_str = relative.to_string_lossy().to_string();

        if path.is_dir() {
            writer.add_directory(&relative_str, options)?;
            walk_and_add(writer, source_root, &path, options)?;
        } else if path.is_file() {
            writer.start_file(&relative_str, options)?;
            let mut input = File::open(&path)?;
            std::io::copy(&mut input, writer)?;
        }
    }
    Ok(())
}

/// Extract a zip archive to `dest_dir`, creating it if it doesn't exist.
///
/// Rejects entries whose path would write outside `dest_dir`
/// ([`Error::UnsafePath`]).
///
/// ```no_run
/// altair_compress::unzip("/tmp/proj.zip", "/tmp/restored").unwrap();
/// ```
pub fn unzip(archive: impl AsRef<Path>, dest_dir: impl AsRef<Path>) -> Result<()> {
    let dest = dest_dir.as_ref();
    create_dir_all(dest)?;
    let archive_file = File::open(archive.as_ref())?;
    let mut zip_archive = ::zip::ZipArchive::new(BufReader::new(archive_file))?;

    for i in 0..zip_archive.len() {
        let mut entry = zip_archive.by_index(i)?;
        let entry_path = entry
            .enclosed_name()
            .ok_or_else(|| Error::UnsafePath {
                path: Path::new(entry.name()).to_path_buf(),
            })?;
        let safe_dest = safe_path::resolve(dest, &entry_path)?;

        if entry.is_dir() {
            create_dir_all(&safe_dest)?;
        } else {
            if let Some(parent) = safe_dest.parent() {
                create_dir_all(parent)?;
            }
            let mut out = BufWriter::new(File::create(&safe_dest)?);
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            out.write_all(&buf)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_fixture_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::File::create(root.join("a.txt"))
            .unwrap()
            .write_all(b"alpha")
            .unwrap();
        fs::File::create(root.join("sub").join("b.txt"))
            .unwrap()
            .write_all(b"beta")
            .unwrap();
        dir
    }

    #[test]
    fn round_trip_directory_tree() {
        let src = make_fixture_dir();
        let work = TempDir::new().unwrap();
        let archive = work.path().join("out.zip");
        let restored = work.path().join("restored");

        zip_dir(src.path(), &archive).unwrap();
        unzip(&archive, &restored).unwrap();

        let a_content = fs::read_to_string(restored.join("a.txt")).unwrap();
        let b_content = fs::read_to_string(restored.join("sub").join("b.txt")).unwrap();
        assert_eq!(a_content, "alpha");
        assert_eq!(b_content, "beta");
    }

    #[test]
    fn non_directory_source_rejected() {
        let work = TempDir::new().unwrap();
        let file = work.path().join("not_a_dir.txt");
        fs::File::create(&file).unwrap();
        let archive = work.path().join("out.zip");
        match zip_dir(&file, &archive) {
            Err(Error::InvalidSource { reason, .. }) => {
                assert!(reason.contains("not a directory"));
            }
            other => panic!("expected InvalidSource, got {other:?}"),
        }
    }

    #[test]
    fn unzip_rejects_parent_dir_entry() {
        // The `zip` crate's `enclosed_name()` already rejects ".." entries,
        // so we get UnsafePath back when we hand it one. Build a zip with
        // such an entry manually.
        let work = TempDir::new().unwrap();
        let archive = work.path().join("malicious.zip");
        {
            let writer = BufWriter::new(File::create(&archive).unwrap());
            let mut zip_writer = ::zip::ZipWriter::new(writer);
            zip_writer
                .start_file("../escape.txt", SimpleFileOptions::default())
                .unwrap();
            zip_writer.write_all(b"oops!").unwrap();
            zip_writer.finish().unwrap();
        }

        let restored = work.path().join("restored");
        match unzip(&archive, &restored) {
            Err(Error::UnsafePath { .. }) => {}
            other => panic!("expected UnsafePath, got {other:?}"),
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-compress --lib zip`
Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-compress/src/zip.rs
git commit -m "feat(compress): zip_dir / unzip with DEFLATE and zip-slip protection"
```

---

## Phase 7: tarball recipe (tar.gz convenience)

### Task 7.1: Write `tarball.rs` with tests

**Files:**
- Create: `crates/altair-compress/src/tarball.rs`

- [ ] **Step 1: Write the file**

```rust
//! tar.gz convenience recipes (combined tar + gzip).

use crate::error::{Error, Result};
use crate::safe_path;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use std::fs::{File, create_dir_all};
use std::io::{BufReader, BufWriter};
use std::path::Path;

/// Archive `source_dir` and gzip-compress the result in one step,
/// producing a `.tar.gz` file at `output`.
///
/// Equivalent to chaining [`crate::tar_dir`] followed by [`crate::compress_file`],
/// but writes directly to the output stream without an intermediate file.
///
/// Returns [`Error::InvalidSource`] if `source_dir` isn't a directory.
///
/// ```no_run
/// altair_compress::tar_gz_dir("./my-project", "/tmp/proj.tar.gz").unwrap();
/// ```
pub fn tar_gz_dir(source_dir: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<()> {
    let source = source_dir.as_ref();
    if !source.is_dir() {
        return Err(Error::InvalidSource {
            path: source.to_path_buf(),
            reason: "not a directory".into(),
        });
    }
    let output_file = File::create(output.as_ref())?;
    let writer = BufWriter::new(output_file);
    let gz = GzEncoder::new(writer, Compression::default());
    let mut builder = ::tar::Builder::new(gz);
    builder.append_dir_all("", source)?;
    builder.finish()?;
    Ok(())
}

/// Extract a `.tar.gz` archive to `dest_dir`.
///
/// Streams through `flate2::read::GzDecoder` into `tar::Archive` without an
/// intermediate uncompressed file. Applies zip-slip protection to every
/// entry ([`Error::UnsafePath`]).
///
/// ```no_run
/// altair_compress::untar_gz("/tmp/proj.tar.gz", "/tmp/restored").unwrap();
/// ```
pub fn untar_gz(archive: impl AsRef<Path>, dest_dir: impl AsRef<Path>) -> Result<()> {
    let dest = dest_dir.as_ref();
    create_dir_all(dest)?;
    let archive_file = File::open(archive.as_ref())?;
    let gz = GzDecoder::new(BufReader::new(archive_file));
    let mut archive = ::tar::Archive::new(gz);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?.into_owned();
        let safe_dest = safe_path::resolve(dest, &entry_path)?;
        entry.unpack(&safe_dest)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_fixture_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::File::create(root.join("a.txt"))
            .unwrap()
            .write_all(b"alpha")
            .unwrap();
        fs::File::create(root.join("sub").join("b.txt"))
            .unwrap()
            .write_all(b"beta")
            .unwrap();
        dir
    }

    #[test]
    fn round_trip_directory_tree() {
        let src = make_fixture_dir();
        let work = TempDir::new().unwrap();
        let archive = work.path().join("out.tar.gz");
        let restored = work.path().join("restored");

        tar_gz_dir(src.path(), &archive).unwrap();
        untar_gz(&archive, &restored).unwrap();

        let a_content = fs::read_to_string(restored.join("a.txt")).unwrap();
        let b_content = fs::read_to_string(restored.join("sub").join("b.txt")).unwrap();
        assert_eq!(a_content, "alpha");
        assert_eq!(b_content, "beta");
    }

    #[test]
    fn malformed_gzip_yields_io_error() {
        let work = TempDir::new().unwrap();
        let bogus = work.path().join("bogus.tar.gz");
        File::create(&bogus)
            .unwrap()
            .write_all(b"this is not a gzip stream")
            .unwrap();
        let restored = work.path().join("restored");
        match untar_gz(&bogus, &restored) {
            Err(Error::Io(_)) => {}
            other => panic!("expected Io, got {other:?}"),
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p altair-compress --lib tarball`
Expected: 2 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-compress/src/tarball.rs
git commit -m "feat(compress): tar_gz_dir / untar_gz convenience recipes"
```

---

## Phase 8: Prelude

### Task 8.1: Write `prelude.rs`

**Files:**
- Create: `crates/altair-compress/src/prelude.rs`

- [ ] **Step 1: Write the file**

```rust
//! Common imports for users of this crate.
//!
//! ```
//! use altair_compress::prelude::*;
//! ```
//!
//! The prelude exposes the 8 recipe functions, [`Error`], and [`Result`]. It
//! does **not** glob-export the underlying libraries (`flate2`, `tar`, `zip`)
//! — those are available as fully-qualified paths
//! (`altair_compress::flate2::...`, etc.) for power users.

pub use crate::{
    Error, Result, compress_file, decompress_file, tar_dir, tar_gz_dir, untar, untar_gz, unzip,
    zip_dir,
};
```

- [ ] **Step 2: Verify the whole crate now compiles and all tests pass**

Run: `cargo test -p altair-compress --lib`
Expected: 22+ tests pass across error, safe_path, gzip, tar, zip, tarball.

Run: `cargo test -p altair-compress --doc`
Expected: doc tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-compress/src/prelude.rs
git commit -m "feat(compress): add prelude module"
```

---

## Phase 9: Integration tests, example, README

### Task 9.1: Integration test

**Files:**
- Create: `crates/altair-compress/tests/integration.rs`

- [ ] **Step 1: Write the file**

```rust
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
    // Same fixture should round-trip identically through zip and tar paths.
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
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p altair-compress --tests`
Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-compress/tests/integration.rs
git commit -m "test(compress): integration tests for round-trips across formats"
```

### Task 9.2: Example binary

**Files:**
- Create: `crates/altair-compress/examples/basic.rs`

- [ ] **Step 1: Write the file**

```rust
//! Run with: `cargo run --example basic -p altair-compress`

use altair_compress::prelude::*;
use std::fs;
use std::io::Write;

fn main() -> anyhow::Result<()> {
    // Create a small fixture in a temp dir
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
```

- [ ] **Step 2: Build and run**

Run: `cargo run -p altair-compress --example basic`
Expected: prints four lines (sizes for tar/zip/tar.gz and the restored README content); exits 0.

- [ ] **Step 3: Commit**

```bash
git add crates/altair-compress/examples/basic.rs
git commit -m "docs(compress): basic usage example"
```

### Task 9.3: Full README

**Files:**
- Modify: `crates/altair-compress/README.md`

- [ ] **Step 1: Replace stub with the full README**

````markdown
# altair-compress

Path-based recipes for gzip, tar, and zip — with zip-slip protection and a `tar.gz` convenience for the dominant Unix archive format.

Part of the [altair-rs](https://github.com/jasoet/altair-rs) workspace.

## Add to your project

```bash
cargo add altair-compress
```

The underlying libraries (`flate2`, `tar`, `zip`) are re-exported at the crate root — you don't need to add them separately.

## Quick start — archive and extract a directory

```rust,no_run
use altair_compress::{tar_gz_dir, untar_gz};

# fn run() -> altair_compress::Result<()> {
tar_gz_dir("./my-project", "/tmp/backup.tar.gz")?;
untar_gz("/tmp/backup.tar.gz", "/tmp/restored")?;
# Ok(()) }
```

## All eight recipes

```rust,no_run
use altair_compress::prelude::*;

# fn run() -> altair_compress::Result<()> {
// Single-file gzip
compress_file("data.bin", "data.bin.gz")?;
decompress_file("data.bin.gz", "data.bin")?;

// Directory archiving (no compression)
tar_dir("./src", "/tmp/src.tar")?;
untar("/tmp/src.tar", "/tmp/restored")?;

// Directory archiving with DEFLATE compression
zip_dir("./src", "/tmp/src.zip")?;
unzip("/tmp/src.zip", "/tmp/restored")?;

// Combined tar + gzip (most common Unix archive)
tar_gz_dir("./src", "/tmp/src.tar.gz")?;
untar_gz("/tmp/src.tar.gz", "/tmp/restored")?;
# Ok(()) }
```

## Zip-slip protection

`untar`, `unzip`, and `untar_gz` reject any archive entry whose path would write outside the destination directory. Malicious archives can contain entries like `../etc/passwd` or absolute paths; we return [`Error::UnsafePath`] before any writing happens.

```rust,no_run
# use altair_compress::{untar, Error};
match untar("malicious.tar", "/tmp/safe") {
    Err(Error::UnsafePath { path }) => eprintln!("rejected: {path:?}"),
    Ok(()) => println!("extracted"),
    Err(other) => eprintln!("other error: {other}"),
}
```

## Need custom compression levels or builder access?

The recipes use sensible defaults — gzip level 6, DEFLATE for zip. For more control, drop to the re-exported libraries:

```rust,no_run
use altair_compress::flate2::{Compression, write::GzEncoder};
use std::fs::File;
use std::io::Write;

# fn run() -> std::io::Result<()> {
let out = File::create("max.gz")?;
let mut encoder = GzEncoder::new(out, Compression::new(9));  // maximum compression
encoder.write_all(b"important payload")?;
encoder.finish()?;
# Ok(()) }
```

`altair_compress::tar` and `altair_compress::zip` are likewise available for power users.

## Constraints (v0.1)

- **Synchronous only.** Compression is CPU-bound; wrap with `tokio::task::spawn_blocking` if you need it off the async runtime.
- **No symlink preservation.** Symlinks during archive creation are followed; the target's content is recorded.
- **No archive password support.** ZIP encryption is intentionally not exposed.
- **Path safety, not canonicalization.** Zip-slip protection checks for `..` components and absolute paths; it doesn't resolve symlinks at intermediate components. Standard practice for the threat model.

## Error reference

| Variant | When |
|---|---|
| `Error::Io` | File open/read/write failed; most underlying flate2/tar errors surface here |
| `Error::Compression` | Non-`io` failure inside `flate2` or `tar` |
| `Error::Zip` | Error from the `zip` library (CRC, unsupported method, etc.) |
| `Error::UnsafePath` | Archive entry would extract outside the destination directory |
| `Error::InvalidSource` | `*_dir` recipe was given something that isn't a directory |

## License

[MIT](../../LICENSE)
````

- [ ] **Step 2: Verify doc tests still pass**

Run: `cargo test -p altair-compress --doc`
Expected: doc tests pass (lib.rs + recipe doc tests + prelude.rs example).

- [ ] **Step 3: Commit**

```bash
git add crates/altair-compress/README.md
git commit -m "docs(compress): complete README with examples and error reference"
```

---

## Phase 10: Tracker, root README, CI gate

### Task 10.1: Update porting tracker

**Files:**
- Modify: `docs/porting-tracker.md`

- [ ] **Step 1: Add `altair-compress` to the published-set table**

In the "Published crates" table at the top of the file, after the `altair-base32` row, add:

```markdown
| [`altair-compress`](https://crates.io/crates/altair-compress) | 0.1.2 (date TBD on publish) |
```

(The version will be the current workspace shared version. Replace the date after publish.)

- [ ] **Step 2: Add a release notes bullet**

In the "Release notes" list:

```markdown
- **`altair-compress` 0.1.2** (date TBD on publish) — Recipes for gzip, tar, zip, and tar.gz with zip-slip protection. Re-exports `flate2`, `tar`, `zip` for power users.
```

- [ ] **Step 3: Remove `altair-compress` from the "Awaiting Demand" section**

Find and delete:

```markdown
| `compress` | `altair-compress` | 💤 Deferred | `flate2`, `tar`, `zip` | Direct equivalents exist |
```

And add to the Starter Set table (which is now the "delivered crates" section):

```markdown
| `compress` | `altair-compress` | ✅ Done | `flate2`, `tar`, `zip` | Path-based recipes with zip-slip protection |
```

- [ ] **Step 4: Bump "Last updated"**

Replace the existing "Last updated" line with today's date.

- [ ] **Step 5: Commit**

```bash
git add docs/porting-tracker.md
git commit -m "docs: add altair-compress to porting tracker"
```

### Task 10.2: Add to root README

**Files:**
- Modify: `README.md` (workspace root)

- [ ] **Step 1: Add a row to the crate table**

After the `altair-base32` row, add:

```markdown
| [`altair-compress`](crates/altair-compress) | gzip + tar + zip + tar.gz recipes with zip-slip protection | [![crate](https://img.shields.io/crates/v/altair-compress.svg)](https://crates.io/crates/altair-compress) |
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: list altair-compress in workspace README"
```

### Task 10.3: Full workspace CI gate

- [ ] **Step 1: Run formatter, clippy, all tests, doc build**

Run:
```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --no-deps --all-features
```

Expected: all four commands exit 0.

If clippy flags real issues, fix them. Expected likely flags and their fixes:
- `clippy::cast_possible_truncation` in test code with `(i % 256) as u8` — annotate the test function with `#[allow(clippy::cast_possible_truncation)]`. The truncation is intentional.
- `clippy::needless_continue` if any `_ => continue` arms exist — replace with `_ => {}`.
- `clippy::missing_panics_doc` if any `.expect()` is used — refactor to avoid the expect, or document the panic explicitly with `/// # Panics`.

- [ ] **Step 2: Verify `cargo publish --dry-run`**

Run: `cargo publish --dry-run -p altair-compress`
Expected: "Uploading altair-compress vX.Y.Z", "warning: aborting upload due to dry run".

- [ ] **Step 3: Commit any clippy/fmt fixes**

```bash
git add -p
git commit -m "fix(compress): satisfy clippy/fmt"
```

(Skip if there's nothing to commit.)

---

## Phase 11: Push, PR, publish

### Task 11.1: Push branch and open PR

- [ ] **Step 1: Push**

```bash
git push -u origin feat/altair-compress
```

- [ ] **Step 2: Open PR**

```bash
gh pr create --title "feat(compress): add altair-compress crate" --body "$(cat <<'EOF'
## Summary

Adds the sixth crate to the workspace: \`altair-compress\` — path-based recipes for gzip, tar, zip, and tar.gz with zip-slip protection.

- \`compress_file\` / \`decompress_file\` — single-file gzip
- \`tar_dir\` / \`untar\` — directory archiving (no compression)
- \`zip_dir\` / \`unzip\` — directory archiving with DEFLATE
- \`tar_gz_dir\` / \`untar_gz\` — combined tar + gzip
- Re-exports \`flate2\`, \`tar\`, \`zip\` for power users

Spec: docs/specs/2026-05-28-altair-compress-design.md
Plan: docs/plans/2026-05-28-altair-compress-implementation.md

## Test plan

- [x] 22+ unit tests + 3 integration tests pass
- [x] \`cargo clippy --workspace --all-targets --all-features -- -D warnings\` clean
- [x] \`cargo fmt --all --check\` clean
- [x] \`cargo test --workspace --doc\` clean
- [x] \`cargo publish --dry-run -p altair-compress\` clean
- [x] Example builds and runs (\`cargo run --example basic -p altair-compress\`)
- [ ] CI passes on this PR

## Release implication

Adds one new crate. release-plz publishes on merge at the current workspace version.
EOF
)"
```

- [ ] **Step 3: Wait for CI and merge**

```bash
gh pr checks <pr-number>
gh pr merge <pr-number> --squash --delete-branch
```

### Task 11.2: First publish via release-plz

On merge, the release workflow runs automatically. release-plz will detect a new member and publish it directly (since the workspace version is ahead of crates.io for this crate). A subsequent release PR proposing a workspace-wide patch bump may also open; close it if it doesn't add real value.

- [ ] **Step 1: Verify on crates.io**

```bash
curl -s -H 'User-Agent: altair-rs (jasoet87@gmail.com)' \
  https://crates.io/api/v1/crates/altair-compress | jq -r .crate.max_version
```

Expected: matches the workspace version.

### Task 11.3: Final tracker update

**Files:**
- Modify: `docs/porting-tracker.md`

- [ ] **Step 1: Replace "date TBD on publish" with the actual publish date in both the table row and the release notes bullet.**

- [ ] **Step 2: Commit and push**

```bash
git checkout main && git pull
git checkout -b docs/compress-published
# (edit the file)
git commit -am "docs: record altair-compress publish date"
gh pr create --title "docs: record altair-compress publish date" --body "Trivial tracker update."
```

---

## Self-Review

### Spec Coverage Check

| Spec section | Implemented in task |
|---|---|
| §1 Overview | Plan header + Task 9.3 README |
| §2 Decisions (workspace deps, naming, MSRV, scope, sync, re-exports) | Tasks 1.1, 1.2 |
| §3.1 File layout | Tasks 1.2, 2.1, 3.1, 4.1, 5.1, 6.1, 7.1, 8.1, 9.1, 9.2, 9.3 |
| §3.2 Module responsibilities | Tasks 2.1, 3.1, 4.1, 5.1, 6.1, 7.1 — each module is one file, sole owner of its concern |
| §3.3 Public API surface | Task 1.2 (lib.rs re-exports) + per-recipe tasks |
| §3.4 Error model | Task 2.1 — every variant has a Display test |
| §4.1 Archive entry paths (relative to source) | Task 5.1 `append_dir_all("", source)`; Task 6.1 `strip_prefix`; tests verify it |
| §4.2 Zip-slip mitigation | Task 3.1 `safe_path::resolve` + integration into Tasks 5.1/6.1/7.1 |
| §4.3 Symlink handling | Implementation follows symlinks via std library defaults; documented in Task 9.3 README |
| §4.4 Permissions (silently ignored chmod failures, no tracing dep) | No active code (defaults); documented in README |
| §4.5 Edge cases (empty file, missing source, malformed) | Tasks 4.1, 5.1, 6.1, 7.1 each have an "invalid source" or "malformed" test |
| §5 Testing strategy | Per-module unit tests + Task 9.1 integration + doc tests |
| §6 Cross-crate (no otel, prelude limited) | Task 8.1 prelude has only recipes + Error + Result; no `flate2`/`tar`/`zip` glob |
| §7 Out of scope | Plan adds nothing beyond documented items |
| §8 Risks (zip v9, backend choice, edge cases, hidden levels) | Pin via workspace deps in Task 1.1; README documents recipe defaults and the escape-hatch |
| §9 Versioning | Task 1.2 `version.workspace = true` inherits; release-plz handles tagging |

### Placeholder Scan

- "(date TBD on publish)" appears in Task 10.1 step 1 and step 2 — this is intentional and resolved by Task 11.3 step 1 after publish.
- No "TBD", "TODO", "fill in later", or unhandled edge cases elsewhere.

### Type Consistency

- `Error` enum variants used in tests match Task 2.1: `Io`, `Compression`, `Zip`, `UnsafePath { path }`, `InvalidSource { path, reason }`. ✓
- `Result<T>` alias used consistently. ✓
- `safe_path::resolve` signature `(dest_root: &Path, entry_path: &Path) -> Result<PathBuf>` used identically in Tasks 5.1, 6.1, 7.1. ✓
- `SimpleFileOptions` (from `zip::write`) used consistently in Task 6.1. ✓
- The module/library name clash (`mod zip` + `pub use ::zip`) is handled by always using `::tar::Builder`, `::tar::Archive`, `::zip::ZipWriter` (fully-qualified to the external crate) inside the source. ✓

No drift identified.

---

## Execution Handoff

**Plan complete and saved to `docs/plans/2026-05-28-altair-compress-implementation.md`. Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** — execute tasks in this session via executing-plans, batch with checkpoints

Pick when ready to start implementation.
