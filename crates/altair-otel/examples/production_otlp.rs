//! Production-like setup: ship spans + metrics to an OTLP/gRPC collector
//! over the network.
//!
//! Run with a local collector listening on the default port:
//!   `cargo run --example production_otlp -p altair-otel`
//!
//! Or point at a remote collector:
//!   `OTLP_ENDPOINT=http://collector.observability:4317 cargo run --example production_otlp -p altair-otel`
//!
//! If nothing is listening, the OTLP batch exporter will retry and eventually
//! drop the spans silently. The example still runs to completion.

#![allow(clippy::used_underscore_binding)] // `_enter` is the standard tracing-span guard name

use altair_otel::Exporter;
use altair_otel::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let endpoint =
        std::env::var("OTLP_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:4317".to_string());

    Config::builder()
        .service_name("payments-api")
        .service_version("1.4.2")
        .resource_attribute("env", "prod")
        .resource_attribute("region", "us-east-1")
        .otlp_endpoint(endpoint.clone())
        .exporter(Exporter::Otlp)
        .log_format(altair_otel::LogFormat::Json)
        .build()
        .unwrap()
        .init()?;

    info!(endpoint = %endpoint, "telemetry pipeline up");

    // Emit a span tree like a real request would.
    let span = tracing::info_span!("http.request", method = "POST", route = "/charge");
    let _enter = span.enter();
    info!(amount_cents = 4999_i64, "received charge request");
    tokio::time::sleep(Duration::from_millis(20)).await;
    info!(charge_id = "ch_abc123", "charge succeeded");
    drop(_enter);

    // And a metric.
    meter()
        .u64_counter("payments.charges.total")
        .build()
        .add(1, &[KeyValue::new("status", "succeeded")]);

    // Give the periodic exporter time to flush a batch.
    tokio::time::sleep(Duration::from_millis(200)).await;
    shutdown();
    Ok(())
}
