//! Common imports for users of this crate.
//!
//! ```
//! use altair_otel::prelude::*;
//! ```

pub use crate::{Config, ConfigBuilder, Error, Result, meter, shutdown};
pub use crate::{Span, debug, error, info, instrument, span, trace, warn};
pub use opentelemetry::metrics::{Counter, Histogram, Meter, UpDownCounter};
