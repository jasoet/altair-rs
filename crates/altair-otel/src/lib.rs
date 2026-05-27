//! One-call OpenTelemetry setup for tokio applications.
//!
//! After calling [`Config::init`], all `tracing::info!`, `tracing::warn!`, and
//! `#[tracing::instrument]`-decorated functions emit OTLP spans + logs. Use
//! [`meter`] to obtain an OpenTelemetry [`Meter`]
//! for explicit metric instrumentation.
//!
//! # Example
//!
//! ```no_run
//! # async fn run() -> altair_otel::Result<()> {
//! altair_otel::Config::from_env()?.init()?;
//!
//! tracing::info!(user_id = 42, "request received");
//!
//! let counter = altair_otel::meter().u64_counter("requests.total").build();
//! counter.add(1, &[]);
//!
//! altair_otel::shutdown();
//! # Ok(()) }
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

pub mod config;
mod error;
mod globals;
mod init;

pub mod prelude;

pub use config::{Config, ConfigBuilder, Exporter, LogFormat};
pub use error::{Error, Result};
pub use globals::{meter, shutdown};

// Re-exports for one-dep ergonomics
pub use opentelemetry::metrics::{Counter, Histogram, Meter, UpDownCounter};
pub use tracing::{self, Span, debug, error, info, instrument, span, trace, warn};
