//! Run with: `cargo run --example basic -p altair-otel`
//!
//! Uses the stdout exporter so no collector is required.

use altair_otel::prelude::*;
use altair_otel::{Config, Exporter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Config::builder()
        .service_name("basic-example")
        .service_version("0.1.0")
        .exporter(Exporter::Stdout)
        .build()
        .unwrap()
        .init()?;

    info!(user_id = 42, "request received");

    let counter = meter().u64_counter("requests.total").build();
    counter.add(1, &[]);
    counter.add(1, &[]);

    do_work().await;

    shutdown();
    Ok(())
}

#[instrument]
async fn do_work() {
    info!("doing work");
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
}
