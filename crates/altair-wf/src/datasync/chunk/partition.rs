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
    /// Build a partition `[start, end)` without validating ordering.
    /// The chunked-sync helper does not check `start < end`; if your
    /// partitioner could produce a swapped range, use
    /// [`Partition::try_new`] to fail at the boundary instead of
    /// downstream.
    pub fn new(start: K, end: K) -> Self {
        Self { start, end }
    }

    /// Build a partition `[start, end)`, returning
    /// [`Error::InvalidInput`](crate::error::Error::InvalidInput) when
    /// `end <= start`. Use this in partitioner implementations that
    /// want defensive validation.
    pub fn try_new(start: K, end: K) -> crate::error::Result<Self>
    where
        K: PartialOrd + std::fmt::Debug,
    {
        if end <= start {
            return Err(crate::error::Error::InvalidInput(format!(
                "partition end ({end:?}) must be > start ({start:?})"
            )));
        }
        Ok(Self { start, end })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_new_rejects_zero_width_or_inverted_range() {
        assert!(Partition::try_new(10_i64, 10).is_err());
        assert!(Partition::try_new(10_i64, 5).is_err());
    }

    #[test]
    fn try_new_accepts_normal_range() {
        let p = Partition::try_new(0_i64, 10).unwrap();
        assert_eq!((p.start, p.end), (0, 10));
    }

    #[test]
    fn partition_serde_round_trip() {
        let p = Partition::new(0_i64, 10);
        let json = serde_json::to_string(&p).unwrap();
        let back: Partition<i64> = serde_json::from_str(&json).unwrap();
        assert_eq!((back.start, back.end), (0, 10));
    }

    #[test]
    fn partition_result_serde_round_trip() {
        let pr = PartitionResult::<i64> {
            start: 100,
            end: 200,
            fetched: 5,
            inserted: 4,
            updated: 1,
            skipped: 0,
        };
        let json = serde_json::to_string(&pr).unwrap();
        let back: PartitionResult<i64> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.start, 100);
        assert_eq!(back.end, 200);
        assert_eq!(back.fetched, 5);
        assert_eq!(back.inserted, 4);
        assert_eq!(back.updated, 1);
    }

    #[test]
    fn sync_result_serde_round_trip_with_partitions() {
        let summary = SyncResult::<i64> {
            job_name: "j".into(),
            total_partitions: 1,
            total_fetched: 5,
            total_inserted: 5,
            total_updated: 0,
            total_skipped: 0,
            partitions: vec![PartitionResult {
                start: 0,
                end: 10,
                fetched: 5,
                inserted: 5,
                updated: 0,
                skipped: 0,
            }],
            deferred: true,
        };
        let json = serde_json::to_string(&summary).unwrap();
        let back: SyncResult<i64> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.job_name, "j");
        assert!(back.deferred);
        assert_eq!(back.total_fetched, 5);
        assert_eq!(back.partitions.len(), 1);
        assert_eq!(back.partitions[0].end, 10);
    }
}
