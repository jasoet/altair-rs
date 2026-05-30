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
        check_entry_links(&entry, &entry_path)?;
        entry.unpack(&safe_dest)?;
    }
    Ok(())
}

fn check_entry_links<R: std::io::Read>(
    entry: &::tar::Entry<'_, R>,
    entry_path: &Path,
) -> Result<()> {
    let header = entry.header();
    if header.entry_type().is_symlink() || header.entry_type().is_hard_link() {
        let link = header
            .link_name()
            .map_err(|_| Error::UnsafePath {
                path: entry_path.to_path_buf(),
            })?
            .ok_or_else(|| Error::UnsafePath {
                path: entry_path.to_path_buf(),
            })?;
        safe_path::check_link_target(entry_path, &link)?;
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
    fn untar_rejects_symlink_with_absolute_target() {
        let work = TempDir::new().unwrap();
        let archive = work.path().join("symlink.tar");

        let mut header = ::tar::Header::new_ustar();
        header.set_path("link").unwrap();
        header.set_entry_type(::tar::EntryType::Symlink);
        header.set_link_name("/etc").unwrap();
        header.set_size(0);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_cksum();

        {
            let file = File::create(&archive).unwrap();
            let mut builder = ::tar::Builder::new(file);
            builder.append(&header, &[][..]).unwrap();
            builder.finish().unwrap();
        }

        let restored = work.path().join("restored");
        match untar(&archive, &restored) {
            Err(Error::UnsafePath { .. }) => {}
            other => panic!("expected UnsafePath, got {other:?}"),
        }
    }

    #[test]
    fn untar_rejects_symlink_with_parent_traversal() {
        let work = TempDir::new().unwrap();
        let archive = work.path().join("symlink.tar");

        let mut header = ::tar::Header::new_ustar();
        header.set_path("link").unwrap();
        header.set_entry_type(::tar::EntryType::Symlink);
        header.set_link_name("../escape").unwrap();
        header.set_size(0);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_cksum();

        {
            let file = File::create(&archive).unwrap();
            let mut builder = ::tar::Builder::new(file);
            builder.append(&header, &[][..]).unwrap();
            builder.finish().unwrap();
        }

        let restored = work.path().join("restored");
        match untar(&archive, &restored) {
            Err(Error::UnsafePath { .. }) => {}
            other => panic!("expected UnsafePath, got {other:?}"),
        }
    }

    #[test]
    fn untar_rejects_parent_dir_entry() {
        // The tar crate rejects `..` paths in `Header::set_path`, so we cannot
        // produce a malicious archive via the high-level API. Hand-craft a
        // minimal valid ustar header pointing at `../escape.txt` and write it
        // raw to verify our extract-side mitigation works.
        use std::io::Write;

        let work = TempDir::new().unwrap();
        let archive = work.path().join("malicious.tar");

        let mut header = [0u8; 512];
        let name = b"../escape.txt";
        header[..name.len()].copy_from_slice(name);
        header[100..108].copy_from_slice(b"0000644\0");
        header[108..116].copy_from_slice(b"0000000\0");
        header[116..124].copy_from_slice(b"0000000\0");
        header[124..136].copy_from_slice(b"00000000005\0"); // size=5
        header[136..148].copy_from_slice(b"00000000000\0"); // mtime
        // Checksum field is initially eight spaces while we sum the rest.
        header[148..156].fill(b' ');
        header[156] = b'0'; // type = regular file
        header[257..263].copy_from_slice(b"ustar\0");

        // Compute the checksum and write it in octal at offset 148.
        let checksum: u32 = header.iter().map(|&b| u32::from(b)).sum();
        let checksum_str = format!("{checksum:06o}\0 ");
        header[148..156].copy_from_slice(checksum_str.as_bytes());

        let mut tar_data = Vec::with_capacity(2048);
        tar_data.extend_from_slice(&header);
        tar_data.extend_from_slice(b"oops!");
        // Pad to 512-byte boundary then add two empty blocks (archive terminator).
        while !tar_data.len().is_multiple_of(512) {
            tar_data.push(0);
        }
        tar_data.extend_from_slice(&[0u8; 1024]);

        File::create(&archive)
            .unwrap()
            .write_all(&tar_data)
            .unwrap();

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
