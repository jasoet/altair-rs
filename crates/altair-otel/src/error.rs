//! Errors from `OTel` initialization.

use thiserror::Error;

/// Errors from [`crate::Config::init`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// Failed to build the OTLP exporter or tracer/meter/logger provider.
    #[error("otel exporter: {0}")]
    Exporter(String),

    /// The global tracing subscriber was already set.
    #[error("tracing subscriber already initialized")]
    AlreadyInitialized,

    /// An environment variable required by [`crate::Config::from_env`] is missing or malformed.
    #[error("env config: {key} - {message}")]
    EnvConfig {
        /// The offending env var key.
        key: String,
        /// Reason it was rejected.
        message: String,
    },

    /// Invalid configuration supplied to the builder.
    #[error("invalid configuration: {0}")]
    Configuration(String),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_config_renders() {
        let e = Error::EnvConfig {
            key: "OTEL_EXPORTER_OTLP_ENDPOINT".into(),
            message: "not a valid URL".into(),
        };
        assert!(e.to_string().contains("OTEL_EXPORTER_OTLP_ENDPOINT"));
        assert!(e.to_string().contains("not a valid URL"));
    }
}
