//! Common imports for users of this crate.
//!
//! ```no_run
//! use altair_rest::prelude::*;
//!
//! # async fn run() -> altair_rest::Result<()> {
//! let client = Client::builder().build()?;
//! let _ = client.get("https://example.com").send().await?;
//! # Ok(()) }
//! ```

pub use crate::{Client, ClientBuilder, Error, Result};
