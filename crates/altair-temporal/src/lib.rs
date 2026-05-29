//! Stable facade over the pre-1.0 `temporalio-*` Rust SDK.
//!
//! Owns config, client/worker builders, retry-policy and schedule builders,
//! error classification, and workflow-ID-encoded payload helpers. The five
//! `temporalio-*` crates are re-exported at the crate root so consumers
//! depend on `altair-temporal` alone.
//!
//! See the crate README for usage.

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod config;
mod error;
mod retry;

pub use config::{Config, TlsConfig};
pub use error::{BoxError, Error, Result};
pub use retry::{RetryPolicy, RetryPolicyBuilder};

// Underlying-lib re-exports
pub use ::temporalio_sdk;
pub use ::temporalio_sdk_core;
pub use ::temporalio_client;
pub use ::temporalio_common;
pub use ::temporalio_macros;
