//! Shows the tracing spans emitted by retry: a top-level `retry` span with
//! `retry.name` and `retry.max_attempts`, plus a nested `retry.attempt` span
//! per try. Final outcome (`success`/`permanent`/`exhausted`/`cancelled`)
//! emits as a tracing event with `retry.elapsed_ms` and `retry.attempts`.
//!
//! Run with: `RUST_LOG=info cargo run --example tracing_output -p altair-retry`

use altair_retry::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,altair_retry=debug")),
        )
        .init();

    let attempts = Arc::new(AtomicU32::new(0));
    let counter = attempts.clone();

    let cfg = Config::builder()
        .name("db.write")
        .max_retries(4)
        .initial_interval(Duration::from_millis(50))
        .multiplier(2.0)
        .jitter(false)
        .build();

    let result: Result<&'static str> = retry(cfg, move || {
        let counter = counter.clone();
        async move {
            let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
            if n < 3 {
                Err(std::io::Error::other("transient connection error"))
            } else {
                Ok("write committed")
            }
        }
    })
    .await;

    println!("\nfinal: {result:?}");
    Ok(())
}
