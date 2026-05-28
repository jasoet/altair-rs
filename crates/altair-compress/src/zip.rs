//! Directory archiving via `zip` (DEFLATE compression).

use crate::error::{Error, Result};
use crate::safe_path;
use std::fs::{File, create_dir_all};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use ::zip::write::SimpleFileOptions;

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
        // The `zip` crate's `enclosed_name()` may reject ".." entries directly;
        // either way, we get UnsafePath back.
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
