//! Graceful shutdown pattern: wait for a signal (Ctrl-C / SIGTERM), then
//! drain pending telemetry via `shutdown()` before the process exits.
//!
//! This is the canonical "production" pattern for any service.
//!
//! Run with: `cargo run --example graceful_shutdown -p altair-otel`
//! Press Ctrl-C to trigger the shutdown.

use altair_otel::Exporter;
use altair_otel::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Config::builder()
        .service_name("graceful-demo")
        .exporter(Exporter::Stdout)
        .build()
        .unwrap()
        .init()?;

    info!("service started, press Ctrl-C to exit");

    // In a real app: spawn background tasks, run an HTTP server, etc.
    // Here we just emit a metric every second so you see something
    // happening.
    let counter = meter().u64_counter("heartbeats.total").build();
    let mut ticker = tokio::time::interval(Duration::from_millis(500));

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                counter.add(1, &[]);
                info!("heartbeat");
            }
            _ = tokio::signal::ctrl_c() => {
                info!("received Ctrl-C, shutting down...");
                break;
            }
        }
    }

    // Flush spans + metrics to the configured exporter before exit.
    shutdown();
    info!("shutdown complete");
    Ok(())
}
