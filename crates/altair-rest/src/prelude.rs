//! Common imports for users of this crate.
//!
//! ```no_run
//! use altair_rest::prelude::*;
//!
//! # async fn run() -> altair_rest::Result<()> {
//! let client = Client::builder().build()?;
//! let response = client.get("https://example.com").send().await?;
//! let status: StatusCode = response.status();
//! # let _ = status;
//! # Ok(()) }
//! ```

pub use crate::{
    Client, ClientBuilder, Error, HeaderMap, HeaderName, HeaderValue, Method, Response, Result,
    StatusCode,
};
