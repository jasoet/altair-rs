//! Crockford Base32 encode/decode with `u64` helpers and Mod-37,5 check digit.
//!
//! Implements [Crockford Base32](https://www.crockford.com/base32.html):
//!
//! - **Lenient decode** — case-insensitive; `I`/`L` decode as `1`, `O` decodes as `0`;
//!   hyphens (`-`) in encoded input are stripped silently.
//! - **`u64` helpers** — fixed 13-character, zero-padded output suitable for
//!   sortable ID generation (ULID/CUID style).
//! - **Mod-37,5 check digit** — opt-in via [`encode_with_check`] / [`decode_with_check`].
//!
//! # Example
//!
//! ```
//! use altair_base32::{encode, decode};
//!
//! let ciphertext = encode(b"hello world");
//! let plaintext = decode(&ciphertext).unwrap();
//! assert_eq!(plaintext, b"hello world");
//! ```
//!
//! # Example — `u64` for sortable IDs
//!
//! ```
//! use altair_base32::{encode_u64, decode_u64};
//!
//! let id = encode_u64(1_234_567_890);
//! assert_eq!(id.len(), 13);
//! assert_eq!(decode_u64(&id).unwrap(), 1_234_567_890);
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]

mod bytes;
mod check;
mod error;
mod u64;

pub mod prelude;

pub use bytes::{decode, encode};
pub use check::{decode_with_check, encode_with_check};
pub use error::{Error, Result};
pub use u64::{decode_u64, encode_u64};
