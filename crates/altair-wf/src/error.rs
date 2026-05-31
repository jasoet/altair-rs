//! Crate-wide error type for `altair-wf`.

use thiserror::Error;

/// Errors returned by `altair-wf` patterns.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// User-supplied input failed validation. The payload's own
    /// `validate()` impl, or one of the pattern-level invariants
    /// (non-empty task list, DAG cycle, duplicate node name).
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// One step inside a pattern returned a failure and the pattern was
    /// configured with `fail_fast`. The position is the 0-based index
    /// inside the task list (or the DAG node name for DAG patterns).
    #[error("pattern stopped at step {position}: {reason}")]
    PatternStopped {
        /// Index or name of the failing step.
        position: String,
        /// Reason — task-reported error message or activity failure.
        reason: String,
    },

    /// The underlying activity invocation failed at the SDK boundary
    /// (network error, activity panic, timeout, retry exhaustion).
    #[error("activity '{activity}' failed: {source}")]
    Activity {
        /// The activity name as it appears in Temporal.
        activity: String,
        /// Underlying SDK error.
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// Convenience result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_input_renders() {
        let e = Error::InvalidInput("tasks must be non-empty".into());
        assert_eq!(e.to_string(), "invalid input: tasks must be non-empty");
    }

    #[test]
    fn pattern_stopped_renders_position_and_reason() {
        let e = Error::PatternStopped {
            position: "3".into(),
            reason: "task reported failure".into(),
        };
        let s = e.to_string();
        assert!(s.contains("step 3"));
        assert!(s.contains("task reported failure"));
    }
}
