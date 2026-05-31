//! Crate-wide error type for `altair-server`.

use thiserror::Error;

/// Errors returned by `altair-server` operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to bind the TCP listener.
    #[error("failed to bind {addr}: {source}")]
    Bind {
        /// The address we attempted to bind.
        addr: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// I/O error during the serve loop.
    #[error("server I/O: {0}")]
    Io(#[from] std::io::Error),

    /// Builder rejected a configuration value (bad bind address, etc.).
    #[error("server configuration: {0}")]
    Configuration(String),

    /// In-flight requests did not drain within the configured
    /// `shutdown_timeout` after the shutdown future resolved.
    #[error("graceful shutdown timed out after {0:?}")]
    ShutdownTimeout(std::time::Duration),
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_error_renders_addr_and_source() {
        let e = Error::Bind {
            addr: "0.0.0.0:8080".into(),
            source: std::io::Error::other("address in use"),
        };
        let s = e.to_string();
        assert!(s.contains("0.0.0.0:8080"));
        assert!(s.contains("address in use"));
    }

    #[test]
    fn io_error_renders() {
        let io = std::io::Error::other("disk full");
        let e: Error = io.into();
        assert!(e.to_string().contains("disk full"));
    }

    #[test]
    fn configuration_error_renders() {
        let e = Error::Configuration("invalid port".into());
        assert_eq!(e.to_string(), "server configuration: invalid port");
    }
}
