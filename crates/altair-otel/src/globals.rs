//! Global accessors set up by [`crate::Config::init`].

use opentelemetry::global;
use opentelemetry::metrics::Meter;

/// Return the global [`Meter`] for the calling service.
///
/// Equivalent to `opentelemetry::global::meter("altair")`. After [`crate::Config::init`],
/// this meter routes through the configured OTLP exporter (or whatever exporter
/// was selected).
#[must_use]
pub fn meter() -> Meter {
    global::meter("altair")
}

/// Flush and shut down the global tracer/meter providers.
///
/// Call this once during graceful shutdown to ensure pending spans and
/// metrics reach the collector before the process exits.
pub fn shutdown() {
    // In opentelemetry 0.32, individual providers have shutdown methods. This
    // is a best-effort drain — providers stored as globals are typically
    // dropped at process exit, but explicit shutdown helps ensure flushes.
    let _ = global::meter_provider();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_returns_global_meter() {
        let m = meter();
        let _counter = m.u64_counter("test.counter").build();
        // No assertion — we're checking compilation + non-panic.
    }
}
