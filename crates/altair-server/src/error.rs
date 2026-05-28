//! Error types.

use thiserror::Error;

/// Result type for altair-server operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for altair-server.
#[derive(Error, Debug)]
pub enum Error {
    /// Generic error message.
    #[error("{0}")]
    Other(String),
}
