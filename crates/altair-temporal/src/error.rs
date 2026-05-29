//! Error type for altair-temporal.
//!
//! Source fields are boxed (`Box<dyn Error + Send + Sync>`) so this
//! type's public surface stays stable across temporalio-sdk majors —
//! the SDK's concrete error types are exactly what this crate shields.
//! Consumers can downcast through `err.source()` when they need the
//! original.

use thiserror::Error;

/// Boxed error source.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// All errors that may surface from `altair-temporal`.
#[derive(Debug, Error)]
pub enum Error {
    /// Could not establish a gRPC connection to the Temporal server.
    #[error("failed to connect to temporal at {host}")]
    Connect {
        /// The host URL that the connect attempt targeted.
        host: String,
        /// Underlying error (typically a tonic/transport error).
        #[source]
        source: BoxError,
    },

    /// A client-side operation failed (start workflow, get handle, etc.).
    #[error("temporal client error")]
    Client(#[source] BoxError),

    /// A worker-side operation failed (build, poll, shutdown).
    #[error("temporal worker error")]
    Worker(#[source] BoxError),

    /// A schedule operation (create/update/delete) failed.
    #[error("temporal schedule error")]
    Schedule(#[source] BoxError),

    /// The supplied `Config` is invalid.
    #[error("invalid configuration: {0}")]
    Configuration(String),
}

/// Shorthand `Result` parameterised over the crate's `Error`.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Construct a `Connect` error from a host and any boxable source.
    pub fn connect(host: impl Into<String>, source: impl Into<BoxError>) -> Self {
        Self::Connect {
            host: host.into(),
            source: source.into(),
        }
    }

    /// Construct a `Client` error from any boxable source.
    pub fn client(source: impl Into<BoxError>) -> Self {
        Self::Client(source.into())
    }

    /// Construct a `Worker` error from any boxable source.
    pub fn worker(source: impl Into<BoxError>) -> Self {
        Self::Worker(source.into())
    }

    /// Construct a `Schedule` error from any boxable source.
    pub fn schedule(source: impl Into<BoxError>) -> Self {
        Self::Schedule(source.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_carries_host_and_source() {
        let err = Error::connect("http://localhost:7233", "boom".to_string());
        assert_eq!(
            err.to_string(),
            "failed to connect to temporal at http://localhost:7233"
        );
        assert!(matches!(err, Error::Connect { .. }));
    }

    #[test]
    fn client_wraps_source() {
        let err = Error::client("io issue".to_string());
        assert_eq!(err.to_string(), "temporal client error");
    }

    #[test]
    fn configuration_carries_message() {
        let err = Error::Configuration("task_queue is required".to_string());
        assert_eq!(
            err.to_string(),
            "invalid configuration: task_queue is required"
        );
    }
}
