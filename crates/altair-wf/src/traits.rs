//! `TaskInput` / `TaskOutput` — the marker traits every payload that flows
//! through an `altair-wf` pattern must implement.

use crate::error::Result;

/// Marker trait for activity inputs handled by `altair-wf` patterns.
///
/// The default `validate` returns `Ok(())` — override it to enforce
/// payload-level invariants before the pattern hands the input to the
/// activity. Patterns call `validate()` at the entry point and return
/// [`crate::Error::InvalidInput`] on any error.
///
/// Unlike the Go counterpart, this trait has no `activity_name()` method
/// — Rust Temporal activities are dispatched by typed function reference,
/// not by string name. Pass the activity reference to the pattern helper
/// alongside the input.
pub trait TaskInput {
    /// Validate the input. Returning `Err` aborts the pattern with
    /// [`crate::Error::InvalidInput`].
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

/// Marker trait for activity outputs handled by `altair-wf` patterns.
///
/// Patterns inspect every result via [`Self::is_success`] to maintain
/// `total_success` / `total_failed` counters and to decide whether to
/// abort on a `fail_fast` strategy. [`Self::error`] is consulted to fill
/// the `reason` field on [`crate::Error::PatternStopped`].
pub trait TaskOutput {
    /// `true` if the activity completed its business logic successfully.
    fn is_success(&self) -> bool;

    /// Human-readable error description when `is_success()` returns
    /// `false`. Default `None`.
    fn error(&self) -> Option<&str> {
        None
    }
}
