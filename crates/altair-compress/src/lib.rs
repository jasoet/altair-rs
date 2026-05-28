//! Path-based recipes for gzip, tar, and zip — plus the dominant tar.gz combo.
//!
//! Wraps battle-tested libraries ([`flate2`], [`tar`], [`zip`]) with smart
//! defaults, typed errors, and zip-slip protection. The underlying libraries
//! are re-exported at the crate root for power users who need custom
//! compression levels, builder-style archive construction, or other features
//! beyond the recipes.
//!
//! # Example
//!
//! ```no_run
//! use altair_compress::{tar_gz_dir, untar_gz};
//!
//! # fn run() -> altair_compress::Result<()> {
//! tar_gz_dir("./my-project", "/tmp/backup.tar.gz")?;
//! untar_gz("/tmp/backup.tar.gz", "/tmp/restored")?;
//! # Ok(()) }
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod error;
mod gzip;
mod safe_path;
mod tar;
mod tarball;
mod zip;

pub mod prelude;

pub use error::{Error, Result};
pub use gzip::{compress_file, decompress_file};
pub use tar::{tar_dir, untar};
pub use tarball::{tar_gz_dir, untar_gz};
pub use zip::{unzip, zip_dir};

// Re-exports for one-dep ergonomics. Note `flate2`, `tar`, and `zip` are also
// the names of our own modules — those modules are `mod` (private) above; the
// `pub use` here exports the *external* crates under the same names.
pub use ::flate2;
pub use ::tar;
pub use ::zip;
