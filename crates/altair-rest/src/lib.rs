//! Resilient HTTP client built on `reqwest` with retry and `OTel`-aware
//! tracing middleware baked in.
//!
//! Wraps `reqwest_middleware::ClientWithMiddleware` with a sensible-default
//! middleware chain so each outgoing request automatically gets exponential
//! backoff on transient failures plus a per-request tracing span. The
//! underlying `reqwest` and `reqwest_middleware` crates are re-exported
//! at the crate root so consumers don't need to add them separately.
//!
//! # Example
//!
//! ```no_run
//! use altair_rest::Client;
//!
//! # async fn run() -> altair_rest::Result<()> {
//! let client = Client::builder()
//!     .base_url("https://api.example.com")?
//!     .bearer_token("secret-token")
//!     .build()?;
//!
//! let response = client.get("/users/42").send().await?;
//! # let _ = response;
//! # Ok(()) }
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod client;
mod config;
mod error;
mod json;

pub mod prelude;

pub use client::Client;
pub use config::ClientBuilder;
pub use error::{Error, Result};

// Re-exports for one-dep ergonomics
pub use ::reqwest;
pub use ::reqwest_middleware;
