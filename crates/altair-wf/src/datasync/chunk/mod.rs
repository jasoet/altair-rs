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
//! - [`ChunkedSyncSummary<K>`] — the workflow-level summary, including
//!   the `deferred` flag the caller checks to decide whether to issue
//!   continue-as-new.
//! - [`chunked_sync_run`] — the SDK-agnostic orchestration helper.
//! - [`iterate_partitions`] — pure-Rust partition walker for non-Temporal
//!   callers.
//!
//! # Sketch
//!
//! Inside a `#[workflow]` body, dispatch each step as an activity and
//! let `chunked_sync_run` drive the loop. The caller issues
//! continue-as-new at the workflow boundary when `result.deferred` is
//! true — the helper cannot do it from a generic function because the
//! workflow type is not in scope there.
//!
//! ```ignore
//! use altair_wf::datasync::chunk::{
//!     chunked_sync_run, ChunkedSyncConfig, Cursor, Partition, PartitionResult,
//! };
//!
//! // ctx: &WorkflowContext<MyWf>, opts: ActivityOptions, input: MyInput
//! let list = || async move {
//!     ctx.start_activity(MyActs::list_partitions, (), opts.clone()).await
//!         .map_err(|e| altair_wf::Error::activity("list_partitions", e))
//! };
//! let run = move |p: Partition<i64>| async move {
//!     ctx.start_activity(MyActs::run_partition, p, opts.clone()).await
//!         .map_err(|e| altair_wf::Error::activity("run_partition", e))
//! };
//! let cursor = Cursor::Some {
//!     read: || async {
//!         ctx.start_activity(MyActs::read_cursor, input.job.clone(), opts.clone()).await
//!             .map_err(|e| altair_wf::Error::activity("read_cursor", e))
//!     },
//!     advance: move |end: i64| async move {
//!         ctx.start_activity(MyActs::advance_cursor, end, opts.clone()).await
//!             .map_err(|e| altair_wf::Error::activity("advance_cursor", e))
//!     },
//! };
//! let result = chunked_sync_run(
//!     ChunkedSyncConfig::new(&input.job).max_partitions_per_execution(100),
//!     list, run, cursor,
//!     |d| async move { ctx.timer(d).await; },
//! ).await?;
//! if result.deferred {
//!     ctx.continue_as_new(&input, Default::default())?;
//!     unreachable!();
//! }
//! ```

mod iterate;
mod partition;
mod partitioner;
mod sync;
mod tracker;

pub use iterate::iterate_partitions;
pub use partition::{ChunkedSyncSummary, Partition, PartitionResult};
pub use partitioner::Partitioner;
pub use sync::{ChunkedSyncConfig, Cursor, chunked_sync_run};
pub use tracker::ProgressTracker;
