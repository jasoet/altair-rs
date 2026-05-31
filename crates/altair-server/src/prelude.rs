//! Common imports for users of this crate.
//!
//! ```no_run
//! use altair_server::prelude::*;
//!
//! async fn hello() -> &'static str {
//!     "hello"
//! }
//!
//! # async fn run() -> altair_server::Result<()> {
//! let app = Router::new().route("/", get(hello));
//! let server = Server::builder()
//!     .bind_addr("127.0.0.1:0")
//!     .merge(app)
//!     .build()
//!     .await?;
//! # let _ = server;
//! # Ok(()) }
//! ```

pub use crate::{Error, Result, Server, ServerBuilder, shutdown_signal};

// Re-export the axum types a typical handler needs so consumers don't
// have to remember the `altair_server::axum::extract::*` path. The
// underlying `axum` re-export at the crate root remains available for
// less common items.
pub use crate::axum::Router;
pub use crate::axum::extract::{Path, Query, State};
pub use crate::axum::http::StatusCode;
pub use crate::axum::response::IntoResponse;
pub use crate::axum::routing::{any, delete, get, patch, post, put};
pub use crate::axum::{Extension, Json};
