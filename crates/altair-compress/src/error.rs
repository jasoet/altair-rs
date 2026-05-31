//! Crate-wide error type for `altair-compress`.

use thiserror::Error;

/// Errors returned by `altair-compress` recipes.
#[derive(Debug, Error)]
#[non_exhaustive]
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
