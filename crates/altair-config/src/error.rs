//! Config loading and validation errors.

use thiserror::Error;

/// Errors returned by config loaders.
#[derive(Debug, Error)]
pub enum Error {
    /// I/O error while reading a config file.
    #[error("config I/O: {0}")]
    Io(#[from] std::io::Error),

    /// Underlying parse/merge error from `figment`.
    #[error("config parse: {0}")]
    Parse(#[from] figment::Error),

    /// Validation failed.
    #[error("config validation failed: {0}")]
    Validation(#[from] validator::ValidationErrors),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_renders() {
        let io = std::io::Error::other("nope");
        let e: Error = io.into();
        assert!(e.to_string().contains("nope"));
    }
}
