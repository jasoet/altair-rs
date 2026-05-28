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
#[allow(clippy::cast_possible_truncation)] // the i % 256 truncation is intentional
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
        match result {
            Err(crate::error::Error::Io(_)) => {}
            other => panic!("expected Io, got {other:?}"),
        }
    }
}
