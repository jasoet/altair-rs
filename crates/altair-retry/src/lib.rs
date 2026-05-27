//! Async retry with exponential backoff and automatic tracing.
//!
//! Each retry attempt runs inside a `tracing::span!` so it appears in
//! distributed traces. If `altair-otel` is initialized in the same process,
//! retries flow to OTLP automatically.
//!
//! # Example
//!
//! ```no_run
//! use altair_retry::{retry, Config};
//! use std::time::Duration;
//!
//! # async fn run() -> altair_retry::Result<()> {
//! # async fn ping() -> std::io::Result<()> { Ok(()) }
//! let cfg = Config::builder()
//!     .name("db.connect")
//!     .max_retries(3)
//!     .initial_interval(Duration::from_millis(100))
//!     .build();
//!
//! retry(cfg, || async { ping().await }).await?;
//! # Ok(()) }
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod config;
mod error;
mod retry;

pub mod prelude;

pub use config::{Config, ConfigBuilder};
pub use error::{Error, PermanentError, Result};
pub use retry::retry;

// Re-exports for one-dep ergonomics
pub use tokio_util::sync::CancellationToken;
