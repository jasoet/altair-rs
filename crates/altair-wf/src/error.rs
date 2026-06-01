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
    ///
    /// The `source` field carries the original `Send + Sync + 'static`
    /// error — production observability code can `downcast_ref` to
    /// recover the concrete `ActivityError` for retry classification,
    /// payload inspection, or structured logging. Sanitise before
    /// emitting to external observability sinks if payload privacy is
    /// a concern. Prefer the [`Self::activity`] constructor over the
    /// struct literal — it handles the `Box::new(...)` for you.
    #[error("activity '{activity}' failed: {source}")]
    Activity {
        /// The activity name as it appears in Temporal.
        activity: String,
        /// Underlying SDK error. `'static` bound so callers can
        /// downcast via `source.downcast_ref::<ActivityError>()`.
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

impl Error {
    /// Build an [`Error::Activity`] without the `Box::new(...)` /
    /// `.into()` ceremony at every call site. Most workflows wrap
    /// their `start_activity(...).await.map_err(...)` chain in
    /// `Error::activity("MyActivities::method", e)` — without this
    /// helper the construction is verbose enough to be transcription-
    /// error prone.
    ///
    /// # Examples
    ///
    /// ```
    /// use altair_wf::Error;
    /// let io = std::io::Error::other("network");
    /// let err = Error::activity("MyActivities::ping", io);
    /// assert!(matches!(err, Error::Activity { .. }));
    /// ```
    #[must_use]
    pub fn activity<E>(name: impl Into<String>, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Activity {
            activity: name.into(),
            source: Box::new(source),
        }
    }
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

    #[test]
    fn activity_source_can_be_downcast_to_concrete_type() {
        // Regression: `Error::Activity::source` must carry the
        // `'static` bound so production observability code can
        // recover the original error type via `downcast_ref`. Without
        // it, operators inspecting an incident get only `.to_string()`.
        let io = std::io::Error::other("network blip");
        let err = Error::activity("MyActs::ping", io);
        match err {
            Error::Activity { source, .. } => {
                let recovered = source
                    .downcast_ref::<std::io::Error>()
                    .expect("downcast to io::Error");
                assert_eq!(recovered.kind(), std::io::ErrorKind::Other);
                assert!(recovered.to_string().contains("network blip"));
            }
            other => panic!("expected Error::Activity, got {other:?}"),
        }
    }
}
