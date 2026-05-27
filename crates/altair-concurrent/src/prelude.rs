//! Common imports for users of this crate.
//!
//! ```
//! use altair_concurrent::prelude::*;
//! ```

pub use crate::{
    BoxedError, CancellationToken, Error, Executor, PartialExecutor, PartialResults, Result,
    TaskMap, execute_concurrently,
};
