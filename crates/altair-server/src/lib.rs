//! Axum + tower-http convenience layer.
//!
//! Wraps an `axum::Router` with a sensible-default middleware stack
//! (tracing, request-id propagation, request timeout), a built-in `/health`
//! endpoint, and SIGINT/SIGTERM-aware graceful shutdown. The underlying
//! `axum`, `tower`, and `tower-http` crates are re-exported at the crate
//! root so consumers don't need to add them as separate dependencies.
//!
//! # Example
//!
//! ```no_run
//! use altair_server::Server;
//! use altair_server::axum::routing::get;
//!
//! # async fn run() -> altair_server::Result<()> {
//! Server::builder()
//!     .bind_addr("0.0.0.0:3000")
//!     .route("/", get(|| async { "hello" }))
//!     .build()
//!     .await?
//!     .run()
//!     .await
//! # }
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod builder;
mod error;
mod health;
mod middleware;
mod server;
mod shutdown;

pub mod prelude;

pub use builder::ServerBuilder;
pub use error::{Error, Result};
pub use server::Server;
pub use shutdown::shutdown_signal;

// Re-exports for one-dep ergonomics
pub use ::axum;
pub use ::tower;
pub use ::tower_http;
