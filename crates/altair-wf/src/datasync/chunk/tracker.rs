//! [`ProgressTracker`] trait — persists chunked-sync progress so a job
//! can resume after failure (or continue-as-new) instead of restarting
//! at the configured range start.

use async_trait::async_trait;

use crate::error::Result;

/// Persists the last-completed partition end-key so a chunked sync can
/// resume after failure.
///
/// Implementations are **not** provided by `altair-wf` — define your
/// own (e.g. a Postgres-backed `ProgressTracker<i64>`) and wire it
/// into the workflow via the `chunked_sync_run` helper.
#[async_trait]
pub trait ProgressTracker<K>: Send + Sync
where
    K: Send + Sync + 'static,
{
    /// Return the last-completed partition's `end` value for the named
    /// job. `Ok(None)` reports "no cursor yet" — the workflow will
    /// process every partition. Distinct from `K::default()` because
    /// zero may be a meaningful key (e.g. `0i64`).
    ///
    /// The chunked-sync workflow uses the returned key as an inclusive
    /// lower bound on partition `start`: partitions with `start >=
    /// cursor` are processed. The fixture stored here should therefore
    /// be a partition `end` value, not a partition `start`.
    async fn cursor(&self, job_name: &str) -> Result<Option<K>>;

    /// Record that every partition ending at `completed` (inclusive)
    /// has been successfully processed. Implementations **must** be
    /// idempotent — the workflow may retry this call (and the workflow
    /// helper does not roll back the in-memory summary if a retry
    /// observes a duplicate write). An "INSERT … ON CONFLICT DO
    /// NOTHING" pattern or an upsert against a monotone column is the
    /// usual shape.
    async fn advance(&self, job_name: &str, completed: K) -> Result<()>;
}
