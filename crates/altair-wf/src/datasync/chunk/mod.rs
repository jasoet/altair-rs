//! Partitioned, resumable sync built on top of the core datasync
//! primitives.
//!
//! The chunk module lets a sync split its work into ordered, half-open
//! key ranges, optionally remember progress via a [`ProgressTracker`],
//! and hand off work across continue-as-new boundaries when one
//! execution would otherwise span too much workflow history.
//!
//! The core building blocks:
//!
//! - [`Partition<K>`] — a half-open `[start, end)` range.
//! - [`Partitioner<K>`] — returns the partition list for one execution.
//! - [`ProgressTracker<K>`] — persists the last-completed end key so
//!   the next execution can skip past it.
//! - [`SyncResult<K>`] — the workflow-level summary, including the
//!   `deferred` flag the caller checks to decide whether to issue
//!   continue-as-new.
//! - [`chunked_sync_run`] — the SDK-agnostic orchestration helper.
//! - [`iterate_partitions`] — pure-Rust partition walker for non-Temporal
//!   callers.

mod iterate;
mod partition;
mod partitioner;
mod sync;
mod tracker;

pub use iterate::iterate_partitions;
pub use partition::{Partition, PartitionResult, SyncResult};
pub use partitioner::Partitioner;
pub use sync::{ChunkedSyncConfig, Cursor, chunked_sync_run};
pub use tracker::ProgressTracker;
