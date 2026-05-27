//! Run with: `cargo run --example basic -p altair-retry`

use altair_retry::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let attempts = Arc::new(AtomicU32::new(0));
    let a = attempts.clone();

    let result = retry(
        Config::builder()
            .name("flaky.api")
            .max_retries(3)
            .initial_interval(Duration::from_millis(50))
            .build(),
        move || {
            let a = a.clone();
            async move {
                let n = a.fetch_add(1, Ordering::SeqCst) + 1;
                println!("attempt {n}");
                if n < 3 {
                    Err::<&'static str, _>(std::io::Error::other("temporary"))
                } else {
                    Ok("success")
                }
            }
        },
    )
    .await?;

    println!("got: {result}");
    Ok(())
}
