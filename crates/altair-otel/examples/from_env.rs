//! Initialize the `OTel` pipeline from standard `OTEL_*` environment variables.
//! Pattern: useful for 12-factor-style deployments where everything is env-driven.
//!
//! Run with:
//!   `OTEL_SERVICE_NAME=demo-svc cargo run --example from_env -p altair-otel`
//!
//! Optional vars:
//! - `OTEL_SERVICE_VERSION=0.1.0`
//! - `OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4317`
//! - `OTEL_LOG_FORMAT=pretty` or `json`
//! - `RUST_LOG=info,altair_otel=debug`

use altair_otel::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var("OTEL_SERVICE_NAME").is_err() {
        eprintln!("set OTEL_SERVICE_NAME first, e.g.:");
        eprintln!("  OTEL_SERVICE_NAME=demo-svc cargo run --example from_env -p altair-otel");
        return Ok(());
    }

    let cfg = Config::from_env()?;
    cfg.init()?;

    info!("service started");
    do_work();
    shutdown();

    Ok(())
}

#[instrument]
fn do_work() {
    info!("inside do_work — produces a span");
}
