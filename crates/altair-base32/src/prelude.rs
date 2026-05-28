//! Common imports for users of this crate.
//!
//! ```
//! use altair_base32::prelude::*;
//!
//! let id = encode_u64(42);
//! assert_eq!(decode_u64(&id).unwrap(), 42);
//! ```

pub use crate::{
    Error, Result, decode, decode_u64, decode_with_check, encode, encode_u64, encode_with_check,
};
