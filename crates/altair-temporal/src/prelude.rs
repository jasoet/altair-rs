//! Convenience re-exports — one `use altair_temporal::prelude::*;` is
//! enough to write straightforward Temporal workflows and activities.

pub use crate::{
    classify_error, Config, Error, Result, RetryPolicy, RetryPolicyBuilder,
    TlsConfig,
};
