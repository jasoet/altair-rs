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
        // Note: use set_size without header.set_cksum() to create a valid but unusual tar
        let work = TempDir::new().unwrap();
        let archive = work.path().join("malicious.tar");
        {
            // Create a minimal raw tar entry with parent directory traversal
            let mut tar_data = Vec::new();
            // We'll create the tarball manually to bypass tar library's path validation
            use std::io::Write;

            // Simple tar header for ../escape.txt (512 bytes)
            let mut header = [0u8; 512];
            // File name (first 100 bytes)
            let name = b"../escape.txt";
            header[..name.len()].copy_from_slice(name);
            // Mode (offset 100, 8 bytes)
            let mode = b"0000644\0";
            header[100..108].copy_from_slice(mode);
            // Owner uid (offset 108, 8 bytes)
            let uid = b"0000000\0";
            header[108..116].copy_from_slice(uid);
            // Group uid (offset 116, 8 bytes)
            let gid = b"0000000\0";
            header[116..124].copy_from_slice(gid);
            // File size (offset 124, 12 bytes) - 5 bytes = "oops!"
            let size = b"00000000005\0";
            header[124..136].copy_from_slice(size);
            // Modification time (offset 136, 12 bytes)
            let mtime = b"00000000000\0";
            header[136..148].copy_from_slice(mtime);
            // Checksum (offset 148, 8 bytes) - filled with spaces initially
            for i in 148..156 {
                header[i] = b' ';
            }
            // Type flag (offset 156, 1 byte) - '0' for regular file
            header[156] = b'0';
            // Link name (offset 157, 100 bytes) - all zeros
            // ustar indicator (offset 257, 6 bytes)
            let ustar = b"ustar\0";
            header[257..263].copy_from_slice(ustar);

            // Calculate checksum
            let checksum: u32 = header.iter().map(|&b| b as u32).sum();
            let checksum_str = format!("{:06o}\0 ", checksum);
            header[148..156].copy_from_slice(checksum_str.as_bytes());

            tar_data.extend_from_slice(&header);
            tar_data.extend_from_slice(b"oops!");
            // Pad to 512-byte boundary
            while tar_data.len() % 512 != 0 {
                tar_data.push(0);
            }
            // Add two zero blocks to mark end of archive
            tar_data.extend_from_slice(&[0u8; 1024]);

            File::create(&archive).unwrap().write_all(&tar_data).unwrap();
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
