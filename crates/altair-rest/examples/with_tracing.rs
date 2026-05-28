//! Each request emits a `tracing::span!("HTTP {method}")` with attributes.
//! Install a tracing subscriber to see them. With `altair-otel` initialized,
//! these spans flow to OTLP automatically.
//!
//! Run with:
//!   `RUST_LOG=info cargo run --example with_tracing -p altair-rest`

use altair_rest::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let client = Client::builder().base_url("https://httpbin.org")?.build()?;

    let response = client.get("/get").send().await?;
    tracing::info!(status = ?response.status(), "got first response");

    let response = client.get("/uuid").send().await?;
    tracing::info!(status = ?response.status(), "got second response");

    Ok(())
}
