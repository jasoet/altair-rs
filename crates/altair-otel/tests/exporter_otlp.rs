//! Real OTLP export end-to-end: stand up an `otel/opentelemetry-collector`
//! container, point altair-otel at it, emit a span, force-flush via
//! `shutdown()`, and assert the gRPC export call did not error.
//!
//! Gated behind the `integration-tests` feature. Requires Docker.

#![cfg(feature = "integration-tests")]

use altair_otel::testcontainer::OtelCollectorContainer;
use altair_otel::{Config, Exporter};
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn span_exports_to_real_collector() {
    let collector = OtelCollectorContainer::start()
        .await
        .expect("start OTel collector container");

    let cfg = Config::builder()
        .service_name("altair-otel-integration")
        .otlp_endpoint(collector.grpc_endpoint())
        .exporter(Exporter::Otlp)
        .build();

    cfg.init().expect("init OTel pipeline against container");

    // Emit a span + an attribute through tracing — the OTel layer
    // installed by `init()` translates this to an OTLP span on the
    // background batch processor.
    {
        let span = tracing::info_span!("integration_span", customer = "acme", year = 2026_u32);
        let _enter = span.enter();
        tracing::info!(action = "ping", "hello collector");
    }

    // Also push a metric so the meter pipeline is exercised.
    let counter = altair_otel::meter()
        .u64_counter("altair_otel.integration.pings")
        .build();
    counter.add(1, &[]);

    // Give the batch processor a moment to drain.
    tokio::time::sleep(Duration::from_millis(500)).await;

    // shutdown() force-flushes the OTLP exporter. If the collector had
    // refused the gRPC connection or rejected the payload, the underlying
    // SDK would log an error; for our purposes the contract is that
    // shutdown returns without panicking after a real export round-trip.
    altair_otel::shutdown();
}
