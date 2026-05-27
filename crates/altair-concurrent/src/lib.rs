//! Type-safe parallel execution of named async tasks.
//!
//! Provides a [`TaskMap`] for declaring named tasks and an [`execute_concurrently`]
//! entry point that runs them on the tokio runtime with optional cancellation,
//! timeout, and partial-results modes. Each task runs inside its own tracing
//! span so it appears as a separate node in distributed traces.
//!
//! # Example
//!
//! ```no_run
//! use altair_concurrent::{execute_concurrently, TaskMap};
//!
//! # async fn run() -> altair_concurrent::Result<()> {
//! let tasks: TaskMap<String> = TaskMap::new()
//!     .insert("greet", |_| async { Ok::<_, std::io::Error>("hi".to_string()) });
//! let results = execute_concurrently(tasks).await?;
//! assert_eq!(results["greet"], "hi");
//! # Ok(()) }
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod error;
mod executor;
mod task_map;

pub mod prelude;

pub use error::{Error, Result};
pub use executor::{Executor, execute_concurrently};
pub use task_map::TaskMap;

// Re-exports for one-dep ergonomics
pub use tokio_util::sync::CancellationToken;
