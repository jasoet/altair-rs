//! Configure retry behaviour: more attempts, custom backoff.
//!
//! Run with: `cargo run --example with_retry -p altair-rest`

use altair_rest::Client;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::builder()
        .retry_max_attempts(5)
        .retry_initial_interval(Duration::from_millis(50))
        .retry_max_interval(Duration::from_secs(2))
        .build()?;

    // Point at a flaky service. The middleware retries on 5xx and network
    // errors up to 5 times with exponential backoff.
    let response = client.get("https://httpbin.org/status/200").send().await?;
    println!("status: {}", response.status());

    // Disable retries entirely:
    let strict = Client::builder().retry_max_attempts(0).build()?;
    let response = strict.get("https://httpbin.org/status/200").send().await?;
    println!("strict status: {}", response.status());

    Ok(())
}
