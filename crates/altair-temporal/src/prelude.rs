//! Convenience re-exports — one `use altair_temporal::prelude::*;` is
//! enough to write straightforward Temporal workflows and activities.
//!
//! # Notes
//!
//! `Error` and `Result` are deliberately **not** re-exported here. The
//! crate's `Result<T>` is a one-arg alias for
//! `std::result::Result<T, altair_temporal::Error>`, which would shadow
//! `std::result::Result<T, E>` inside `#[activity]` macro expansions
//! (those generate `Result<T, ActivityError>`). Import the alias
//! explicitly when needed: `use altair_temporal::Result as TemporalResult;`.

pub use crate::{
    Client, Config, RetryPolicy, RetryPolicyBuilder, Schedule, ScheduleBuilder, TlsConfig, Worker,
    WorkerBuilder, classify_error, delete_schedule, delete_schedule_if_exists,
};

// SDK macros most consumers reach for when defining workflows / activities.
// Re-exported so users don't need to add `temporalio-macros` to their
// dependencies just to write `#[workflow]` / `#[activity]`.
pub use ::temporalio_macros::{
    activities, activity, init, query, run, signal, update, update_validator, workflow,
    workflow_methods,
};
