//! Convenience re-exports — one `use altair_temporal::prelude::*;` is
//! enough to write straightforward Temporal workflows and activities.

pub use crate::{
    Client, Config, Error, Result, RetryPolicy, RetryPolicyBuilder, Schedule, ScheduleBuilder,
    TlsConfig, Worker, WorkerBuilder, classify_error, delete_schedule,
};
