//! Convenience re-exports — one `use altair_temporal::prelude::*;` is
//! enough to write straightforward Temporal workflows and activities.

pub use crate::{
    Client, Config, Error, Result, RetryPolicy, RetryPolicyBuilder, Schedule, ScheduleBuilder,
    TlsConfig, Worker, WorkerBuilder, classify_error, delete_schedule,
};

// SDK macros most consumers reach for when defining workflows / activities.
// Re-exported so users don't need to add `temporalio-macros` to their
// dependencies just to write `#[workflow]` / `#[activity]`.
pub use ::temporalio_macros::{
    activities, activity, init, query, run, signal, update, update_validator, workflow,
    workflow_methods,
};
