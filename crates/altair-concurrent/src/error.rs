//! Errors produced by parallel task execution.

use thiserror::Error;

/// Errors returned by [`crate::execute_concurrently`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// A task returned an error; remaining tasks were cancelled.
    #[error("task '{name}' failed: {source}")]
    TaskFailed {
        /// The static name of the failing task.
        name: &'static str,
        /// The underlying error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// The cancellation token fired before all tasks completed.
    #[error("execution cancelled")]
    Cancelled,

    /// The configured timeout elapsed before all tasks completed.
    #[error("execution timed out")]
    Timeout,

    /// A task panicked or was cancelled by the runtime.
    #[error("join error: {0}")]
    Join(#[from] tokio::task::JoinError),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_failed_message_includes_name() {
        let err = Error::TaskFailed {
            name: "fetch_user",
            source: "boom".into(),
        };
        assert!(err.to_string().contains("fetch_user"));
        assert!(err.to_string().contains("boom"));
    }

    #[test]
    fn cancelled_renders() {
        assert_eq!(Error::Cancelled.to_string(), "execution cancelled");
    }

    #[test]
    fn timeout_renders() {
        assert_eq!(Error::Timeout.to_string(), "execution timed out");
    }
}
