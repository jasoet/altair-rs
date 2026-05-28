//! Demonstrates every knob `Config::builder()` exposes.
//!
//! Run with: `cargo run --example all_knobs -p altair-retry`

use altair_retry::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cancel = CancellationToken::new();

    let cfg = Config::builder()
        // Appears in tracing spans and error messages
        .name("checkout.charge")
        // Retries after the initial call → max 6 total attempts
        .max_retries(5)
        // First sleep is ~50ms (multiplied by jitter)
        .initial_interval(Duration::from_millis(50))
        // Cap exponential growth at 5s per attempt
        .max_interval(Duration::from_secs(5))
        // Each delay is multiplied by 2.0 (50ms → 100ms → 200ms → ...)
        .multiplier(2.0)
        // Randomize delays to avoid thundering-herd retries
        .jitter(true)
        // Observe an external cancellation signal
        .cancellation_token(cancel)
        .build();

    let attempts = Arc::new(AtomicU32::new(0));
    let counter = attempts.clone();

    let result = retry(cfg, move || {
        let counter = counter.clone();
        async move {
            let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
            println!("attempt {n}");
            if n < 3 {
                Err::<&'static str, _>(std::io::Error::other("not yet ready"))
            } else {
                Ok("charged")
            }
        }
    })
    .await?;

    println!("result: {result}");
    println!("total attempts: {}", attempts.load(Ordering::SeqCst));
    Ok(())
}
