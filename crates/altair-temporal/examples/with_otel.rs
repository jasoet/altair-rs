//! Cross-crate auto-integration: init altair-otel, then Temporal SDK
//! spans flow through automatically.
//!
//! Run with: `cargo run -p altair-temporal --example with_otel`

use altair_otel::{Config as OtelConfig, Exporter};
use altair_temporal::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    OtelConfig::builder()
        .service_name("temporal-demo")
        .service_version("0.1.0")
        .exporter(Exporter::Stdout)
        .build()
        .init()?;

    let cfg = Config {
        task_queue: "altair-demo".to_string(),
        ..Config::default()
    };
    let _client = Client::from_config(&cfg).await?;
    println!("connected; SDK spans now flow through altair-otel exporter");

    altair_otel::shutdown();
    Ok(())
}
