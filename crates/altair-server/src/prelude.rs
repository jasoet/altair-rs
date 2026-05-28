//! Common imports for users of this crate.
//!
//! ```no_run
//! use altair_server::prelude::*;
//!
//! # async fn run() -> altair_server::Result<()> {
//! let server = Server::builder()
//!     .bind_addr("127.0.0.1:0")
//!     .build()
//!     .await?;
//! # let _ = server;
//! # Ok(()) }
//! ```

pub use crate::{Error, Result, Server, ServerBuilder, shutdown_signal};
