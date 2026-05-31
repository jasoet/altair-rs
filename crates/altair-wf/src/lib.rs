//! Reusable Temporal workflow patterns: single-task execution, pipeline
//! (sequential), parallel, loop (and parameterized loop), and DAG with
//! cycle detection.
//!
//! Built on [`altair-temporal`](https://crates.io/crates/altair-temporal),
//! the workspace's stable facade over the pre-1.0 `temporalio-*` Rust SDK.
//!
//! # Design
//!
//! Each pattern is a generic async helper consumed from inside a user's
//! own `#[workflow]` definition — the caller owns the workflow type, the
//! patterns own the orchestration logic. Activity dispatch is **typed**:
//! pass an `ActivityDefinition` (a function reference from an
//! `#[activities]` impl block) to the pattern; do not rely on string
//! activity names. This is a deliberate divergence from the
//! Go [`github.com/jasoet/go-wf`](https://github.com/jasoet/go-wf)
//! library this crate ports, which uses runtime string dispatch.
//!
//! # Example
//!
//! ```no_run
//! # #[cfg(feature = "integration-tests")] {
//! use altair_wf::{TaskInput, TaskOutput, pipeline, PipelineInput};
//!
//! // Your domain types implement the marker traits.
//! #[derive(serde::Serialize, serde::Deserialize, Clone)]
//! struct Step { name: String }
//!
//! impl TaskInput for Step {}
//!
//! #[derive(serde::Serialize, serde::Deserialize, Clone)]
//! struct StepResult { ok: bool, msg: String }
//!
//! impl TaskOutput for StepResult {
//!     fn is_success(&self) -> bool { self.ok }
//!     fn error(&self) -> Option<&str> { (!self.ok).then_some(&self.msg) }
//! }
//! # }
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod dag;
mod error;
mod helpers;
mod patterns;
mod traits;
mod types;

pub mod prelude;

pub use dag::{DAGInput, DAGNode, DAGOutput, NodeResult};
pub use error::{Error, Result};
pub use helpers::{
    DEFAULT_BACKOFF_COEFFICIENT, DEFAULT_INITIAL_INTERVAL_MS, DEFAULT_MAX_INTERVAL_SECS,
    DEFAULT_MAX_RETRY_ATTEMPTS, DEFAULT_START_TO_CLOSE_MINS, FailureStrategy,
    default_activity_options, default_retry_policy, generate_parameter_combinations,
    substitute_template, substitutor_from_fn,
};
pub use patterns::{
    execute, execute_with_timeout, parallel, parameterized_loop, pipeline, run_dag, run_loop,
};
pub use traits::{TaskInput, TaskOutput};
pub use types::{
    LoopInput, LoopOutput, ParallelInput, ParallelOutput, ParameterizedLoopInput, PipelineInput,
    PipelineOutput, Substitutor,
};

// One-dep ergonomics: re-export the altair-temporal types every workflow
// author needs alongside the patterns. Consumers can now write
// `use altair_wf::WorkflowContext;` instead of having to add altair-temporal
// to their `[dependencies]` separately and remember the long path.
pub use altair_temporal;
pub use altair_temporal::RetryPolicy;
pub use altair_temporal::temporalio_sdk::activities::{ActivityContext, ActivityError};
pub use altair_temporal::temporalio_sdk::{ActivityOptions, WorkflowContext, WorkflowResult};
