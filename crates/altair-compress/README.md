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

`untar`, `unzip`, and `untar_gz` reject any archive entry whose path would write outside the destination directory. Malicious archives can contain entries like `../etc/passwd` or absolute paths; we return `Error::UnsafePath` before any writing happens.

```rust,no_run
use altair_compress::{untar, Error};

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
