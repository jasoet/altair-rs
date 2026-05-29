//! Sea-ORM + sqlx convenience layer.
//!
//! Wraps a `sea_orm::DatabaseConnection` (and its underlying sqlx pool) with
//! smart pool defaults, file-based migrations, OTel-aware query tracing, and
//! a closure-style transaction helper. Three backends (Postgres + MySQL +
//! SQLite) are supported behind cargo features.
//!
//! See the crate README for usage.

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod error;

pub use error::{Error, Result};

// Re-exports for one-dep ergonomics
pub use ::sea_orm;
pub use ::sqlx;
