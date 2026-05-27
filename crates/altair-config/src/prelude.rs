//! Common imports for users of this crate.
//!
//! ```
//! use altair_config::prelude::*;
//!
//! #[derive(Debug, Deserialize, Validate)]
//! struct Cfg {
//!     name: String,
//! }
//! ```

pub use crate::{
    Deserialize, Error, Loader, Result, Serialize, Validate, ValidationError, ValidationErrors,
    from_file, from_reader, from_toml_str,
};
