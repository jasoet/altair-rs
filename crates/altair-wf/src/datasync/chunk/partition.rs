//! Partition primitives: half-open key ranges and the per-partition /
//! workflow-level result types.

use serde::{Deserialize, Serialize};

/// Half-open key range `[start, end)` processed as one unit.
///
/// `K` must be `Ord` for the workflow's cursor-filtering pass and for
/// callers who want to walk partitions in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Partition<K> {
    /// Inclusive lower bound.
    pub start: K,
    /// Exclusive upper bound.
    pub end: K,
}

impl<K> Partition<K> {
    /// Build a partition `[start, end)`.
    pub fn new(start: K, end: K) -> Self {
        Self { start, end }
    }
}

/// Per-partition outcome captured in the workflow summary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PartitionResult<K> {
    /// Inclusive lower bound of the partition.
    pub start: K,
    /// Exclusive upper bound of the partition.
    pub end: K,
    /// Number of records the fetcher returned.
    #[serde(default)]
    pub fetched: usize,
    /// Sink-side inserts.
    #[serde(default)]
    pub inserted: usize,
    /// Sink-side updates.
    #[serde(default)]
    pub updated: usize,
    /// Sink-side skips.
    #[serde(default)]
    pub skipped: usize,
}

/// Workflow-level summary aggregating every partition processed during
/// this execution. When the chunked sync defers work via continue-as-new,
/// `partitions` covers only the partitions handled by **this** execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult<K> {
    /// Echo of the job name.
    pub job_name: String,
    /// Count of partitions processed this execution.
    #[serde(default)]
    pub total_partitions: usize,
    /// Sum of `PartitionResult::fetched`.
    #[serde(default)]
    pub total_fetched: usize,
    /// Sum of `PartitionResult::inserted`.
    #[serde(default)]
    pub total_inserted: usize,
    /// Sum of `PartitionResult::updated`.
    #[serde(default)]
    pub total_updated: usize,
    /// Sum of `PartitionResult::skipped`.
    #[serde(default)]
    pub total_skipped: usize,
    /// Per-partition details.
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub partitions: Vec<PartitionResult<K>>,
    /// `true` when the workflow truncated to `max_partitions_per_execution`
    /// and the remaining work must be handled by a continue-as-new run.
    /// Callers should issue `WorkflowContext::continue_as_new` when this
    /// flag is set.
    #[serde(default)]
    pub deferred: bool,
}

// Hand-written `Default` so `SyncResult<K>` is usable when `K` itself
// does not implement `Default` (e.g. when callers parameterise the
// chunked sync over a custom key type).
impl<K> Default for SyncResult<K> {
    fn default() -> Self {
        Self {
            job_name: String::new(),
            total_partitions: 0,
            total_fetched: 0,
            total_inserted: 0,
            total_updated: 0,
            total_skipped: 0,
            partitions: Vec::new(),
            deferred: false,
        }
    }
}
