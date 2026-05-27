//! Type-safe TOML configuration loading with env-var overrides and validation.
//!
//! # Example
//!
//! ```
//! use altair_config::{Deserialize, Validate};
//!
//! #[derive(Debug, Deserialize, Validate)]
//! struct App {
//!     #[validate(range(min = 1, max = 65535))]
//!     port: u16,
//! }
//!
//! let toml = "port = 8080";
//! let cfg: App = altair_config::from_toml_str(toml, "APP_UNUSED_PREFIX_XYZZY").unwrap();
//! assert_eq!(cfg.port, 8080);
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod error;
mod loader;
mod loaders;

pub mod prelude;

pub use error::{Error, Result};
pub use loader::Loader;
pub use loaders::{from_file, from_reader, from_toml_str};

// Re-exports for one-dep ergonomics
pub use serde::{Deserialize, Serialize};
pub use validator::{Validate, ValidationError, ValidationErrors};
