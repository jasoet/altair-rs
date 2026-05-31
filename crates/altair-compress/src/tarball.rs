//! tar.gz convenience recipes (combined tar + gzip).

use crate::error::{Error, Result};
use crate::gzip::DEFAULT_DECOMPRESS_LIMIT_BYTES;
use crate::tar_recipe::unpack_entries;
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
/// Streams through `flate2::read::GzDecoder` into `tar::Archive` without
/// an intermediate uncompressed file. Applies zip-slip protection +
/// symlink target validation to every entry, and caps total decompressed
/// output at [`DEFAULT_DECOMPRESS_LIMIT_BYTES`] (4 GiB). Use
/// [`untar_gz_with_limit`] to override the cap.
///
/// ```no_run
/// altair_compress::untar_gz("/tmp/proj.tar.gz", "/tmp/restored").unwrap();
/// ```
pub fn untar_gz(archive: impl AsRef<Path>, dest_dir: impl AsRef<Path>) -> Result<()> {
    untar_gz_with_limit(archive, dest_dir, DEFAULT_DECOMPRESS_LIMIT_BYTES)
}

/// Like [`untar_gz`] but with a caller-specified `max_total_bytes` cap.
/// Returns [`Error::DecompressionLimit`] if the cap is exceeded.
pub fn untar_gz_with_limit(
    archive: impl AsRef<Path>,
    dest_dir: impl AsRef<Path>,
    max_total_bytes: u64,
) -> Result<()> {
    let dest = dest_dir.as_ref();
    create_dir_all(dest)?;
    let archive_file = File::open(archive.as_ref())?;
    let gz = GzDecoder::new(BufReader::new(archive_file));
    let archive = ::tar::Archive::new(gz);
    unpack_entries(archive, dest, max_total_bytes)
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
