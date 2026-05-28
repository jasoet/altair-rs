//! Realistic pattern for an HTTP API client: retry on transient errors
//! (5xx, network) but immediately fail on permanent ones (4xx, auth).
//!
//! Run with: `cargo run --example flaky_api_pattern -p altair-retry`

use altair_retry::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// Simulated HTTP response.
struct Response {
    status: u16,
    body: &'static str,
}

/// Simulated client whose responses change over time (first 502 twice, then 200).
///
/// In a real codebase this would be `async` and call a real HTTP client; we
/// keep it sync here so the example is self-contained. The `Result` return
/// type is kept (even though we always return `Ok`) to mirror what a real
/// HTTP client would expose, where network errors are possible.
#[allow(clippy::unnecessary_wraps)]
fn make_request(attempt: u32) -> std::io::Result<Response> {
    match attempt {
        1 | 2 => Ok(Response {
            status: 502,
            body: "bad gateway",
        }),
        _ => Ok(Response {
            status: 200,
            body: "{\"id\":42}",
        }),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let attempts = Arc::new(AtomicU32::new(0));
    let counter = attempts.clone();

    let cfg = Config::builder()
        .name("orders.api")
        .max_retries(3)
        .initial_interval(Duration::from_millis(50))
        .jitter(true)
        .build();

    let response: Response = retry(cfg, move || {
        let counter = counter.clone();
        async move {
            let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
            let response = make_request(n).map_err(|e| Box::new(e) as BoxedError)?;
            println!("attempt {n}: HTTP {}", response.status);

            match response.status {
                200..=299 => Ok(response),
                400..=499 => {
                    // Client error — won't get better by retrying.
                    Err(Box::new(PermanentError::wrap(format!(
                        "client error {}: {}",
                        response.status, response.body
                    ))) as BoxedError)
                }
                _ => {
                    // 5xx or any other → transient; backon will sleep and retry.
                    Err(Box::new(std::io::Error::other(format!(
                        "server error {}",
                        response.status
                    ))) as BoxedError)
                }
            }
        }
    })
    .await?;

    println!("final response body: {}", response.body);
    Ok(())
}
