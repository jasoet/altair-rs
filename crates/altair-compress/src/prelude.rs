//! Common imports for users of this crate.
//!
//! ```
//! use altair_compress::prelude::*;
//! ```
//!
//! The prelude exposes the 8 recipe functions, [`Error`], and [`Result`]. It
//! does **not** glob-export the underlying libraries (`flate2`, `tar`, `zip`)
//! — those are available as fully-qualified paths
//! (`altair_compress::flate2::...`, etc.) for power users.

pub use crate::{
    Error, Result, compress_file, decompress_file, tar_dir, tar_gz_dir, untar, untar_gz, unzip,
    zip_dir,
};
