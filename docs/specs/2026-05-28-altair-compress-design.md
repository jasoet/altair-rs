# altair-compress — Design

**Date:** 2026-05-28
**Status:** Draft — awaiting review before implementation planning
**Author:** Jasoet
**Spec type:** Brainstorming output → input to writing-plans

---

## 1. Overview

`altair-compress` provides path-based recipes for the three most common compression/archiving operations in real applications: **gzip** for single-file compression, **tar** for directory archiving without compression, and **zip** for directory archiving with built-in compression. A `tar.gz` convenience recipe covers the dominant Unix-archive format. The crate wraps battle-tested Rust libraries (`flate2`, `tar`, `zip`) with smart defaults, typed errors, and zip-slip protection on extraction.

**One-line product goal:** "Add the dep, compress and archive files — no need to learn three crates' APIs."

The crate also re-exports `flate2`, `tar`, and `zip` at the crate root, so consumers wanting custom compression levels, builder-style archive construction, or any other power-user feature don't need to add those libraries as separate dependencies. The recipes cover the 80% case; the re-exports cover everything else.

## 2. Decisions Locked

| Decision | Choice |
|---|---|
| Scope | gzip (compression) + tar (archiving) + zip (archive + compression) + tar.gz combo |
| Implementation strategy | Wrap `flate2`, `tar`, `zip` directly; no in-crate re-implementation |
| Crate name | `altair-compress` (verified available on crates.io 2026-05-28) |
| API style | Path-based recipes for the 80% case; underlying libraries re-exported at the crate root for power users |
| Sync vs async | **Sync.** CPU-bound work; users who want it off the tokio runtime call `tokio::task::spawn_blocking` themselves |
| Compression level | `flate2::Compression::default()` (level 6) hard-coded in recipes; level overrides require dropping to re-exports |
| Path safety | `untar`/`unzip`/`untar_gz` reject any entry that would write outside the destination directory (zip-slip mitigation) |
| Re-exports | `pub use flate2;`, `pub use tar;`, `pub use zip;` — yes, contrary to `altair-base32`'s "no re-export" stance, because these libraries have rich vocabularies users will want |
| Error type | Single `thiserror` enum: `Io`, `Compression`, `Zip`, `UnsafePath`, `InvalidSource` |
| Dependencies | `flate2`, `tar`, `zip` v8.x, `thiserror` (workspace) — no `tokio`, no `tracing` |
| Symlink handling | Followed during archive creation (target's content stored). Not preserved as symlinks in v0.1. |
| Edition / MSRV | Inherit from workspace (Edition 2024, Rust 1.95) |

## 3. Architecture

### 3.1 File layout

```
crates/altair-compress/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs          # crate root: lints, mod declarations, re-exports, prelude
│   ├── error.rs        # Error enum + Result alias (thiserror)
│   ├── gzip.rs         # compress_file / decompress_file (flate2)
│   ├── tar.rs          # tar_dir / untar (tar)
│   ├── zip.rs          # zip_dir / unzip (zip)
│   ├── tarball.rs      # tar_gz_dir / untar_gz convenience (tar + gzip combo)
│   ├── safe_path.rs    # zip-slip mitigation helper used by all extract paths
│   └── prelude.rs
├── tests/
│   └── integration.rs  # tempdir round-trips for each format + combined tarball
└── examples/
    └── basic.rs        # demonstrate every recipe end-to-end
```

### 3.2 Module responsibilities

- **`error.rs`** — sole owner of the `Error` enum and `Result<T>` alias. Other modules use them and never define their own.
- **`gzip.rs`** — the only place `flate2` is touched for the recipes. `compress_file` wraps `GzEncoder`; `decompress_file` wraps `GzDecoder`.
- **`tar.rs`** — the only place `tar` is touched. `tar_dir` uses `tar::Builder::append_dir_all`; `untar` walks `tar::Archive::entries()` and applies `safe_path::resolve` to each before writing.
- **`zip.rs`** — the only place `zip` is touched. `zip_dir` writes via `zip::ZipWriter`; `unzip` walks `zip::ZipArchive::by_index` and applies `safe_path::resolve`.
- **`tarball.rs`** — composition module. `tar_gz_dir` wraps a `flate2::write::GzEncoder` around a `tar::Builder`; `untar_gz` chains `flate2::read::GzDecoder` into `tar::Archive`. Calls into `tar.rs`'s helpers where possible — does NOT re-implement walking logic.
- **`safe_path.rs`** — `pub(crate) fn resolve(dest_root: &Path, entry: &Path) -> Result<PathBuf>`. Joins, canonicalizes, and verifies the result stays under `dest_root`. Returns `Error::UnsafePath` otherwise.

### 3.3 Public API

```rust
// crate root re-exports
pub use error::{Error, Result};
pub use gzip::{compress_file, decompress_file};
pub use tar::{tar_dir, untar};
pub use zip::{unzip, zip_dir};
pub use tarball::{tar_gz_dir, untar_gz};

// re-exports for the escape-hatch case
pub use flate2;
pub use tar;
pub use zip;

pub mod prelude;
```

Function signatures:

```rust
pub fn compress_file(input: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<()>;
pub fn decompress_file(input: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<()>;

pub fn tar_dir(source_dir: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<()>;
pub fn untar(archive: impl AsRef<Path>, dest_dir: impl AsRef<Path>) -> Result<()>;

pub fn zip_dir(source_dir: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<()>;
pub fn unzip(archive: impl AsRef<Path>, dest_dir: impl AsRef<Path>) -> Result<()>;

pub fn tar_gz_dir(source_dir: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<()>;
pub fn untar_gz(archive: impl AsRef<Path>, dest_dir: impl AsRef<Path>) -> Result<()>;
```

**Note on the `pub use zip;` namespace clash:** the crate has a `src/zip.rs` module and also re-exports the `zip` library. Inside the crate this is resolved by always using fully-qualified paths (`::zip` for the library, `crate::zip` for our module). The re-export at the crate root makes the library available to consumers as `altair_compress::zip`, which is the name users expect.

### 3.4 Error model

```rust
#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("compression: {0}")]
    Compression(String),

    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("entry path escapes destination: {path:?}")]
    UnsafePath { path: std::path::PathBuf },

    #[error("invalid source: {path:?}: {reason}")]
    InvalidSource {
        path: std::path::PathBuf,
        reason: String,
    },
}
```

`Compression(String)` covers `flate2` and `tar` errors that aren't already wrapped in `io::Error`. Most failures in those libraries surface as `io::Error` and flow through `Error::Io` via `#[from]`.

## 4. Behaviour Details

### 4.1 Archive entry paths

`tar_dir("/src/my-project", "/tmp/out.tar")` walks `/src/my-project` recursively and stores entry paths **relative to the source directory**. So `/src/my-project/Cargo.toml` is stored as `Cargo.toml`; `/src/my-project/src/lib.rs` is stored as `src/lib.rs`. `zip_dir` follows the same convention.

Extraction reproduces this structure inside the destination: `untar("/tmp/out.tar", "/dst")` would create `/dst/Cargo.toml` and `/dst/src/lib.rs`.

### 4.2 Zip-slip mitigation

Both `tar` and `zip` formats can carry entry paths like `../etc/passwd` or `/etc/passwd`. The `safe_path::resolve` helper:

1. Joins `dest_root` and `entry_path`.
2. Canonicalizes the result (resolving `.` and `..` components and following any pre-existing symlinks at intermediate components).
3. Verifies the canonical result starts with `dest_root`'s canonical form.
4. Returns `Error::UnsafePath { path: entry_path }` if the check fails.

Applied to every entry during `untar`, `unzip`, `untar_gz`.

### 4.3 Symlink handling (v0.1)

- During archive creation: symlinks are **followed** (the target's content is stored).
- During extraction: any symlink entries in the archive are extracted as **regular files** of the link target's content (since archive creation never produced a real symlink entry). If an archive from another tool contains a symlink entry pointing outside `dest_root`, zip-slip mitigation catches it.

Future versions may add a flag to preserve symlinks as-is.

### 4.4 Permissions

- **Unix:** tar archives preserve mode; extraction restores it. Zip stores `external_attributes` where the writer supports it.
- **Windows:** mode bits are not preserved (NTFS doesn't have Unix permissions).
- Always best-effort: a failure to chmod the extracted file is silently ignored (does not fail the operation). The crate doesn't depend on `tracing`, so failures aren't logged — users who need visibility wrap the call themselves.

### 4.5 Empty / edge cases

- `compress_file` on a 0-byte file → produces a valid gzip stream containing zero compressed bytes; round-trips to a 0-byte file.
- `tar_dir`/`zip_dir` on an empty directory → archive contains zero entries; valid format.
- `tar_dir`/`zip_dir` on a non-directory → `Error::InvalidSource { reason: "not a directory" }`.
- `untar`/`unzip`/`untar_gz` on a missing/malformed archive → `Error::Io` or `Error::Compression` or `Error::Zip` depending on where it fails.

## 5. Testing Strategy

| Layer | Where | Run by |
|---|---|---|
| Unit (inline `#[cfg(test)]`) | each `src/*.rs` | `cargo test --lib` |
| Integration (tempdir round-trips) | `tests/integration.rs` | `cargo test --tests` |
| Doc-tests | every public function | bundled with `cargo test` |
| Example-as-test | `examples/basic.rs` | `cargo build --examples` |

**Specific test obligations:**

| File | Tests |
|---|---|
| `error.rs` | Display rendering of each variant; `UnsafePath` carries the offending path |
| `safe_path.rs` | Accepts entries inside dest; rejects `../escape`, absolute paths, symlinks pointing outside; handles trailing-slash variants |
| `gzip.rs` | Round-trip 1KB file; round-trip empty file; decompress malformed → `Compression`; missing source → `Io` |
| `tar.rs` | Round-trip `{a.txt, sub/b.txt}`; untar with `../escape` entry → `UnsafePath`; non-directory source → `InvalidSource` |
| `zip.rs` | Same shape as `tar.rs`; zip-slip rejection; non-directory source → `InvalidSource` |
| `tarball.rs` | Round-trip a dir tree; `untar_gz` with malformed gzip → `Compression` |
| `tests/integration.rs` | tar.gz round-trip preserves file contents and (on Unix) modes |

**Coverage target:** ≥85% per file. Compression libraries have many error paths that require carefully crafted malformed inputs to exercise — pushing for 90% would be expensive.

**Dev dependency:** `tempfile` (workspace; already used by `altair-config`).

## 6. Cross-Crate Integration

- **No `altair-otel` hookup.** Compression operations are CPU-bound, one-shot, and high-frequency — per-file spans would add noise without insight. Power users wrap with `#[instrument]` themselves.
- **`prelude` module** — 8 recipe functions + `Error` + `Result`. The re-exported third-party libraries (`flate2`, `tar`, `zip`) are NOT in the prelude — they're available at `altair_compress::flate2::...` etc. Glob-importing the prelude shouldn't drag in three external libraries' worth of symbols.

## 7. Out of Scope (v0.1.0)

- `bzip2`, `xz`, `zstd`, `brotli` — revisit if asked
- ZIP encryption / password-protected archives
- Custom compression levels in the recipes — use re-exported streams (e.g., `flate2::write::GzEncoder::new(out, flate2::Compression::new(9))`)
- Async APIs — keep sync; users wrap in `spawn_blocking` if needed
- Symlink preservation in archives
- Extended attributes (xattrs), ACLs, Windows ADS
- Progress reporting / chunked callbacks
- Streaming compress/decompress over `Vec<u8>` — use re-exported streams
- `no_std` support

## 8. Risks & Open Questions

| Item | Risk | Mitigation |
|---|---|---|
| `zip` v9.0 pre-release looming; eventual upgrade may break re-exports | Medium | Pin to v8.x in workspace deps; treat v9 upgrade as our own minor bump when stable |
| `flate2` has multiple backend features (`miniz_oxide` default, `zlib-ng`, etc.) | Low | Use default; don't expose backend choice in public API |
| Zip-slip mitigation has subtle edge cases (case-insensitive macOS filesystems, Windows path quirks, intermediate symlinks) | Medium — security-relevant | Comprehensive `safe_path` tests; use std `canonicalize`; document the canonicalization behavior |
| Recipes hide compression level — surprises users wanting level 9 | Low (by design) | Document in README; recommend dropping to re-exported streams |
| `tarball.rs` composes `tar` and `gzip` — could end up duplicating walking logic from `tar.rs` | Low — caught at implementation | Plan calls `tar.rs`'s helpers; review during implementation |

## 9. Versioning

- Inherits `version.workspace = true` from the workspace, so first publish lands at the current workspace version (currently `0.1.2`). Pre-1.0 versioning policy unchanged: minor bumps may be breaking, patch bumps are additive.
- Re-exports of `flate2`, `tar`, `zip` ARE part of the public API. An upgrade of any of those crates is a potentially breaking change for our consumers; treat as our own minor bump.

## 10. Next Steps

1. **User reviews this spec** (current step)
2. On approval: `writing-plans` skill produces an implementation plan
3. Implementation plan drives: crate scaffolding → per-module TDD → testing → CI → publish at workspace version
