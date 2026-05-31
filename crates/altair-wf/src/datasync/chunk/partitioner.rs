//! [`Partitioner`] trait — generates the list of partitions a chunked
//! sync should process.

use async_trait::async_trait;

use crate::datasync::chunk::partition::Partition;
use crate::error::Result;

/// Generates the partition list for one chunked-sync execution.
///
/// Implementations should be deterministic for a given moment — the
/// returned list is captured in workflow history and shapes activity
/// scheduling.
#[async_trait]
pub trait Partitioner<K>: Send + Sync
where
    K: Send + Sync + 'static,
{
    /// Return the partitions to process. May be empty (no work).
    async fn partitions(&self) -> Result<Vec<Partition<K>>>;
}
