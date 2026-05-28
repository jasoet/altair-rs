//! Attach a `CancellationToken` to a retry config. Firing the token aborts
//! the retry loop — both pre-attempt and during backoff sleep.
//!
//! Run with: `cargo run --example with_cancellation -p altair-retry`

use altair_retry::prelude::*;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let cancel = CancellationToken::new();

    // Simulate cancel from elsewhere — e.g. SIGTERM handler or a watchdog.
    let canceller = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        println!("shutdown signal: cancelling retry");
        canceller.cancel();
    });

    let cfg = Config::builder()
        .name("flaky.service")
        .max_retries(10)
        .initial_interval(Duration::from_millis(100))
        .jitter(false)
        .cancellation_token(cancel)
        .build();

    let started = std::time::Instant::now();
    let result = retry(cfg, || async {
        println!("attempting...");
        Err::<&'static str, _>(std::io::Error::other("transient"))
    })
    .await;

    let elapsed = started.elapsed();
    match result {
        Ok(v) => println!("succeeded after {elapsed:?}: {v}"),
        Err(Error::Cancelled { name }) => {
            println!("retry '{name}' cancelled after {elapsed:?}");
        }
        Err(other) => println!("other: {other}"),
    }
}
