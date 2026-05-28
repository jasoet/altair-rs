//! Demonstrates all three metric instrument types: `Counter`, `UpDownCounter`,
//! and `Histogram`. Uses the stdout exporter so no collector is needed.
//!
//! Run with: `cargo run --example all_metric_types -p altair-otel`

use altair_otel::Exporter;
use altair_otel::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Config::builder()
        .service_name("metrics-demo")
        .exporter(Exporter::Stdout)
        .build()
        .init()?;

    let m = meter();

    // 1) Counter — monotonically increasing (e.g. total requests)
    let requests = m.u64_counter("http.requests.total").build();
    for status in [200_i64, 200, 200, 500, 200] {
        requests.add(
            1,
            &[
                KeyValue::new("route", "/checkout"),
                KeyValue::new("status", status),
            ],
        );
    }

    // 2) UpDownCounter — can go up or down (e.g. in-flight requests)
    let in_flight = m.i64_up_down_counter("http.in_flight").build();
    in_flight.add(1, &[]); // request started
    in_flight.add(1, &[]); // another started
    in_flight.add(-1, &[]); // first one finished

    // 3) Histogram — distributions (e.g. request latency)
    let latency = m.f64_histogram("http.latency.seconds").build();
    for sample_ms in [12.0, 35.0, 8.0, 120.0, 22.0] {
        latency.record(sample_ms / 1000.0, &[KeyValue::new("route", "/checkout")]);
    }

    // Give the periodic exporter a tick to flush.
    tokio::time::sleep(Duration::from_millis(50)).await;
    shutdown();
    Ok(())
}
