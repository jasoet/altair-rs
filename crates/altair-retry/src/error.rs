//! Error types for retry operations.

use thiserror::Error;

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// Errors returned by [`crate::retry`].
#[derive(Debug, Error)]
pub enum Error {
    /// All retry attempts exhausted; final attempt's error is preserved.
    #[error("retry '{name}' exhausted after {attempts} attempts: {source}")]
    Exhausted {
        /// The retry config's name.
        name: String,
        /// Number of attempts made.
        attempts: u32,
        /// The last underlying error.
        #[source]
        source: BoxedError,
    },

    /// The operation returned a [`PermanentError`]; no more retries attempted.
    #[error("retry '{name}' encountered permanent error: {source}")]
    Permanent {
        /// The retry config's name.
        name: String,
        /// The underlying permanent error.
        #[source]
        source: BoxedError,
    },

    /// The cancellation token was triggered.
    #[error("retry '{name}' cancelled")]
    Cancelled {
        /// The retry config's name.
        name: String,
    },
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Marker for non-retryable errors. Wrap an error with [`PermanentError::wrap`]
/// to short-circuit retry — the next attempt is not made and the wrapped
/// error is returned via [`Error::Permanent`].
#[derive(Debug)]
pub struct PermanentError {
    pub(crate) inner: BoxedError,
}

impl PermanentError {
    /// Wrap an error so retry treats it as permanent.
    #[must_use]
    pub fn wrap<E>(e: E) -> Self
    where
        E: Into<BoxedError>,
    {
        Self { inner: e.into() }
    }
}

impl std::fmt::Display for PermanentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl std::error::Error for PermanentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhausted_includes_name_and_count() {
        let e = Error::Exhausted {
            name: "db.connect".into(),
            attempts: 3,
            source: "ENETUNREACH".into(),
        };
        assert!(e.to_string().contains("db.connect"));
        assert!(e.to_string().contains("3 attempts"));
    }

    #[test]
    fn permanent_wrap_preserves_message() {
        let p = PermanentError::wrap("invalid token");
        assert_eq!(p.to_string(), "invalid token");
    }
}
