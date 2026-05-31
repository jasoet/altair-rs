//! `Source` trait — produces records of type `T` from an external system.

use async_trait::async_trait;

use crate::error::Result;

/// Produces records of type `T` from an external system (HTTP API, queue,
/// CSV file, database query, …). Implementations should respect any
/// cancellation signals; the returned vector may be empty but should not
/// represent an error condition.
#[async_trait]
pub trait Source<T>: Send + Sync {
    /// Stable identifier used in logs, traces, and error contexts.
    fn name(&self) -> &str;

    /// Retrieve all records currently visible to this source.
    async fn fetch(&self) -> Result<Vec<T>>;
}
