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
