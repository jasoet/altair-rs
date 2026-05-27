//! Global accessors set up by [`crate::Config::init`].

use opentelemetry::global;
use opentelemetry::metrics::Meter;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use std::sync::{Mutex, OnceLock};

pub(crate) struct InstalledProviders {
    pub tracer: SdkTracerProvider,
    pub meter: SdkMeterProvider,
}

static PROVIDERS: OnceLock<Mutex<Option<InstalledProviders>>> = OnceLock::new();

/// Install the providers built by [`crate::init`]. Returns `false` if already installed
/// or if the slot's lock has been poisoned by a panicking thread.
pub(crate) fn install(providers: InstalledProviders) -> bool {
    let cell = PROVIDERS.get_or_init(|| Mutex::new(None));
    let Ok(mut guard) = cell.lock() else {
        return false;
    };
    if guard.is_some() {
        return false;
    }
    *guard = Some(providers);
    true
}

/// Return the global [`Meter`] for the calling service.
///
/// Equivalent to `opentelemetry::global::meter("altair")`. After [`crate::Config::init`],
/// this meter routes through the configured exporter.
#[must_use]
pub fn meter() -> Meter {
    global::meter("altair")
}

/// Flush and shut down the installed tracer and meter providers.
///
/// Call this once during graceful shutdown so pending spans and metrics
/// reach the collector before the process exits. Idempotent — subsequent
/// calls are no-ops.
pub fn shutdown() {
    let Some(cell) = PROVIDERS.get() else {
        return;
    };
    let Ok(mut guard) = cell.lock() else {
        return;
    };
    let Some(providers) = guard.take() else {
        return;
    };
    if let Err(e) = providers.tracer.shutdown() {
        tracing::warn!("altair-otel: tracer shutdown failed: {e}");
    }
    if let Err(e) = providers.meter.shutdown() {
        tracing::warn!("altair-otel: meter shutdown failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry_sdk::Resource;
    use opentelemetry_sdk::metrics::SdkMeterProvider;
    use opentelemetry_sdk::trace::SdkTracerProvider;

    #[test]
    fn meter_returns_global_meter() {
        let m = meter();
        let _counter = m.u64_counter("test.counter").build();
    }

    #[test]
    fn shutdown_before_init_is_noop() {
        // Idempotent — does nothing if no providers were installed.
        shutdown();
    }

    fn fresh_cell() -> &'static Mutex<Option<InstalledProviders>> {
        // Tests share the same global PROVIDERS. We can't reset it, so we
        // exercise install() in isolation via the public surface only — and
        // tolerate either install outcome depending on test order.
        PROVIDERS.get_or_init(|| Mutex::new(None))
    }

    #[test]
    fn install_returns_false_when_slot_occupied() {
        // Pre-populate the cell with a dummy value, then attempt install.
        let cell = fresh_cell();
        let resource = Resource::builder().build();
        let mut guard = cell.lock().unwrap();
        if guard.is_none() {
            *guard = Some(InstalledProviders {
                tracer: SdkTracerProvider::builder()
                    .with_resource(resource.clone())
                    .build(),
                meter: SdkMeterProvider::builder().with_resource(resource).build(),
            });
        }
        drop(guard);

        let res2 = Resource::builder().build();
        let attempted = install(InstalledProviders {
            tracer: SdkTracerProvider::builder()
                .with_resource(res2.clone())
                .build(),
            meter: SdkMeterProvider::builder().with_resource(res2).build(),
        });
        assert!(!attempted, "install must reject when slot occupied");

        // Drain so shutdown_before_init_is_noop (parallel) still finds slot empty
        // if it runs after this.
        let _ = cell.lock().unwrap().take();
    }
}
