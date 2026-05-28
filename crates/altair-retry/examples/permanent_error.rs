//! `PermanentError::wrap` short-circuits retry — useful for 4xx HTTP
//! responses, auth failures, validation errors, etc. that won't get better
//! by retrying.
//!
//! Run with: `cargo run --example permanent_error -p altair-retry`

use altair_retry::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let attempts = Arc::new(AtomicU32::new(0));
    let counter = attempts.clone();

    let cfg = Config::builder()
        .name("api.call")
        .max_retries(5)
        .initial_interval(Duration::from_millis(10))
        .build();

    let result = retry(cfg, move || {
        let counter = counter.clone();
        async move {
            let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
            println!("attempt {n}: simulating an HTTP 403 response");
            // 403 is a client error — no amount of retrying will fix it.
            Err::<&'static str, _>(PermanentError::wrap("invalid api key"))
        }
    })
    .await;

    match result {
        Ok(value) => println!("succeeded: {value}"),
        Err(Error::Permanent { name, source }) => {
            println!("'{name}' aborted — permanent error: {source}");
            println!(
                "total attempts made: {} (should be 1 — no retries after permanent)",
                attempts.load(Ordering::SeqCst)
            );
        }
        Err(other) => println!("other: {other}"),
    }

    Ok(())
}
