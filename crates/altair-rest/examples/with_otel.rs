//! Cross-crate auto-integration with `altair-otel`.
//!
//! `altair-rest` ships with `reqwest-tracing` middleware enabled by default,
//! so every outbound HTTP request emits an `HTTP {method}` span with
//! `http.method`, `http.url`, `http.status_code` attributes. When
//! `altair-otel` is initialised in the same process, those spans flow to
//! the configured exporter.
//!
//! This example uses the Stdout exporter so no collector is required.
//!
//! Run with: `cargo run --example with_otel -p altair-rest`

use altair_otel::{Config as OtelConfig, Exporter};
use altair_rest::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    OtelConfig::builder()
        .service_name("rest-demo")
        .service_version("0.1.0")
        .exporter(Exporter::Stdout)
        .build()
        .unwrap()
        .init()?;

    let client = Client::builder().base_url("https://httpbin.org")?.build()?;

    // Two requests so we see two HTTP spans in the stdout output.
    let response = client.get("/get").send().await?;
    println!("first response: HTTP {}", response.status());

    let response = client.get("/uuid").send().await?;
    println!("second response: HTTP {}", response.status());

    // Drain pending spans.
    altair_otel::shutdown();
    Ok(())
}
