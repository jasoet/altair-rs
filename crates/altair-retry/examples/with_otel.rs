//! Cross-crate auto-integration with `altair-otel`.
//!
//! When `altair-otel` is initialized in the same process, every
//! `retry.attempt` span from `altair-retry` flows through the configured
//! exporter. This example uses the Stdout exporter so no collector is
//! needed — you'll see the spans printed when you run it.
//!
//! Run with: `cargo run --example with_otel -p altair-retry`

use altair_otel::{Config as OtelConfig, Exporter};
use altair_retry::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Step 1: initialise altair-otel.
    // Stdout exporter prints every span to the console — useful for demos.
    OtelConfig::builder()
        .service_name("retry-demo")
        .service_version("0.1.0")
        .exporter(Exporter::Stdout)
        .build()
        .unwrap()
        .init()?;

    // Step 2: do work that emits retry.attempt spans.
    let attempts = Arc::new(AtomicU32::new(0));
    let counter = attempts.clone();

    let result: Result<&'static str> = retry(
        Config::builder()
            .name("db.query")
            .max_retries(4)
            .initial_interval(Duration::from_millis(50))
            .multiplier(2.0)
            .jitter(false)
            .build(),
        move || {
            let counter = counter.clone();
            async move {
                let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
                tracing::info!(attempt = n, "running query");
                if n < 3 {
                    Err(std::io::Error::other("connection refused"))
                } else {
                    Ok("row fetched")
                }
            }
        },
    )
    .await;

    println!("\nfinal: {result:?}");

    // Step 3: drain pending spans before exit.
    altair_otel::shutdown();
    Ok(())
}
