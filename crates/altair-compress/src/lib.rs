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
mod tarball;

// Internal modules for tar and zip recipes use `_recipe` suffixes so that the
// public re-exports (`pub use ::tar; pub use ::zip;`) below don't clash with
// the internal `mod` names. The recipe functions are re-exported at the crate
// root, so consumers never see these internal names.
#[path = "tar.rs"]
mod tar_recipe;
#[path = "zip.rs"]
mod zip_recipe;

pub mod prelude;

pub use error::{Error, Result};
pub use gzip::{compress_file, decompress_file};
pub use tar_recipe::{tar_dir, untar};
pub use tarball::{tar_gz_dir, untar_gz};
pub use zip_recipe::{unzip, zip_dir};

// Re-exports for one-dep ergonomics — users get `altair_compress::tar::Builder`,
// `altair_compress::zip::ZipWriter`, etc. without adding the libraries separately.
pub use ::flate2;
pub use ::tar;
pub use ::zip;
