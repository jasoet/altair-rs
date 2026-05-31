//! `Sink` trait + [`WriteResult`] — consumes records of type `U` and
//! persists them.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Outcome of a single [`Sink::write`] call. Counts are additive across
/// calls — see [`WriteResult::add`] and [`WriteResult::total`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteResult {
    /// Records that did not exist and were created.
    #[serde(default)]
    pub inserted: usize,
    /// Records that already existed and were updated.
    #[serde(default)]
    pub updated: usize,
    /// Records the sink chose not to write (e.g. duplicates).
    #[serde(default)]
    pub skipped: usize,
}

impl WriteResult {
    /// Merge `other` into `self`.
    pub fn add(&mut self, other: WriteResult) {
        self.inserted += other.inserted;
        self.updated += other.updated;
        self.skipped += other.skipped;
    }

    /// Total records the sink touched (inserted + updated + skipped).
    #[must_use]
    pub fn total(&self) -> usize {
        self.inserted + self.updated + self.skipped
    }
}

/// Consumes records of type `U` and persists them to a destination.
/// Implementations should be batch-friendly; the framework hands the
/// entire mapped batch to a single `write` call.
#[async_trait]
pub trait Sink<U>: Send + Sync {
    /// Stable identifier used in logs, traces, and error contexts.
    fn name(&self) -> &str;

    /// Persist the batch and return the [`WriteResult`] tally.
    async fn write(&self, records: Vec<U>) -> Result<WriteResult>;
}
